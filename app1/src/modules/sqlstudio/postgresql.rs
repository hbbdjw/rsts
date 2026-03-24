use super::models::{MetadataResponse, SqlConnection, TestConnectionRequest, TableDataRequest, ExecuteSqlRequest, ExecuteSqlResponse};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, Row, Column, TypeInfo};
use std::time::{Duration, Instant};
use crate::modules::web::models::PaginationResult;
use futures::TryStreamExt;

pub async fn test_connection(req: &TestConnectionRequest) -> Result<(), String> {
    let mut options = PgConnectOptions::new()
        .host(&req.host)
        .port(req.port)
        .username(&req.username)
        .database(&req.database);

    if let Some(pwd) = &req.password {
        options = options.password(pwd);
    }

    // Set a timeout for the connection test
    options = options.log_statements(log::LevelFilter::Debug);

    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await
        .map_err(|e| format!("Failed to connect to PostgreSQL: {}", e))?;

    // Try a simple query
    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .map_err(|e| format!("Failed to execute query: {}", e))?;

    Ok(())
}

pub async fn get_metadata(
    conn: &SqlConnection,
    action: &str,
    database: Option<&str>,
    schema: Option<&str>,
) -> Result<Vec<MetadataResponse>, String> {
    let mut options = PgConnectOptions::new()
        .host(&conn.host)
        .port(conn.port)
        .username(&conn.username);

    // If listing schemas/tables, connect to the specific database
    // Otherwise use the default database from connection config
    if let Some(db) = database {
        options = options.database(db);
    } else {
        options = options.database(&conn.database);
    }

    if let Some(pwd) = &conn.password {
        options = options.password(pwd);
    }

    options = options.log_statements(log::LevelFilter::Debug);

    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await
        .map_err(|e| format!("Failed to connect to PostgreSQL: {}", e))?;

    let result = match action {
        "databases" => {
            sqlx::query("SELECT datname FROM pg_database WHERE datistemplate = false")
                .fetch_all(&pool)
                .await
                .map_err(|e| format!("Failed to list databases: {}", e))?
                .iter()
                .map(|row| MetadataResponse {
                    name: row.get("datname"),
                    object_type: "database".to_string(),
                })
                .collect()
        },
        "schemas" => {
             sqlx::query("SELECT schema_name FROM information_schema.schemata WHERE schema_name NOT IN ('information_schema', 'pg_catalog', 'pg_toast')")
                .fetch_all(&pool)
                .await
                .map_err(|e| format!("Failed to list schemas: {}", e))?
                .iter()
                .map(|row| MetadataResponse {
                    name: row.get("schema_name"),
                    object_type: "schema".to_string(),
                })
                .collect()
        },
        "tables" => {
            let schema_name = schema.unwrap_or("public");
            sqlx::query("SELECT table_name FROM information_schema.tables WHERE table_schema = $1 AND table_type = 'BASE TABLE'")
                .bind(schema_name)
                .fetch_all(&pool)
                .await
                .map_err(|e| format!("Failed to list tables: {}", e))?
                .iter()
                .map(|row| MetadataResponse {
                    name: row.get("table_name"),
                    object_type: "table".to_string(),
                })
                .collect()
        },
        "views" => {
            let schema_name = schema.unwrap_or("public");
            sqlx::query("SELECT table_name FROM information_schema.views WHERE table_schema = $1")
                .bind(schema_name)
                .fetch_all(&pool)
                .await
                .map_err(|e| format!("Failed to list views: {}", e))?
                .iter()
                .map(|row| MetadataResponse {
                    name: row.get("table_name"),
                    object_type: "view".to_string(),
                })
                .collect()
        },
        "functions" => {
            let schema_name = schema.unwrap_or("public");
            sqlx::query("SELECT routine_name FROM information_schema.routines WHERE routine_schema = $1 AND routine_type = 'FUNCTION'")
                .bind(schema_name)
                .fetch_all(&pool)
                .await
                .map_err(|e| format!("Failed to list functions: {}", e))?
                .iter()
                .map(|row| MetadataResponse {
                    name: row.get("routine_name"),
                    object_type: "function".to_string(),
                })
                .collect()
        },
        _ => return Err(format!("Unsupported action: {}", action)),
    };

    Ok(result)
}

pub async fn get_table_data(
    conn: &SqlConnection,
    req: &TableDataRequest,
) -> Result<PaginationResult, String> {
    let mut options = PgConnectOptions::new()
        .host(&conn.host)
        .port(conn.port)
        .username(&conn.username);

    // If listing schemas/tables, connect to the specific database
    if !req.database.is_empty() {
        options = options.database(&req.database);
    } else {
        options = options.database(&conn.database);
    }

    if let Some(pwd) = &conn.password {
        options = options.password(pwd);
    }

    options = options.log_statements(log::LevelFilter::Debug);

    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await
        .map_err(|e| format!("Failed to connect to PostgreSQL: {}", e))?;

    // Get total count
    let count_sql = format!(
        "SELECT COUNT(*) FROM \"{}\".\"{}\"",
        req.schema, req.table
    );
    let total: i64 = sqlx::query_scalar(&count_sql)
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("Failed to get count: {}", e))?;

    let total = total as u64;

    // Pagination
    let limit = req.page_size;
    let offset = (req.page - 1) * req.page_size;

    let mut sql = format!(
        "SELECT * FROM \"{}\".\"{}\"",
        req.schema, req.table
    );

    if let Some(sort_by) = &req.sort_by {
        if !sort_by.is_empty() {
            let order = req.sort_order.as_deref().unwrap_or("ASC");
            sql.push_str(&format!(" ORDER BY \"{}\" {}", sort_by, order));
        }
    }

    sql.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));

    let rows = sqlx::query(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Failed to fetch data: {}", e))?;

    let mut data = Vec::new();
    for row in rows {
        let mut record = serde_json::Map::new();
        for col in row.columns() {
            let col_name = col.name();
            let type_info = col.type_info();
            let type_name = type_info.name();

            let json_val = match type_name {
                "BOOL" => row.try_get::<Option<bool>, _>(col_name).map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null)),
                "INT2" | "INT4" | "INT8" => row.try_get::<Option<i64>, _>(col_name).map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null)),
                "FLOAT4" | "FLOAT8" => row.try_get::<Option<f64>, _>(col_name).map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null)),
                "VARCHAR" | "TEXT" | "CHAR" | "NAME" => row.try_get::<Option<String>, _>(col_name).map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null)),
                "DATE" => row.try_get::<Option<chrono::NaiveDate>, _>(col_name).map(|v| v.map(|d| serde_json::Value::String(d.format("%Y-%m-%d").to_string())).unwrap_or(serde_json::Value::Null)),
                "TIMESTAMP" => row.try_get::<Option<chrono::NaiveDateTime>, _>(col_name).map(|v| v.map(|dt| serde_json::Value::String(dt.format("%Y-%m-%d %H:%M:%S").to_string())).unwrap_or(serde_json::Value::Null)),
                "TIMESTAMPTZ" => row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(col_name).map(|v| v.map(|dt| serde_json::Value::String(dt.format("%Y-%m-%d %H:%M:%S").to_string())).unwrap_or(serde_json::Value::Null)),
                "JSON" | "JSONB" => row.try_get::<Option<serde_json::Value>, _>(col_name).map(|v| v.unwrap_or(serde_json::Value::Null)),
                 _ => {
                     // Try as string for other types
                     match row.try_get::<Option<String>, _>(col_name) {
                         Ok(v) => Ok(serde_json::to_value(v).unwrap_or(serde_json::Value::Null)),
                         Err(_) => Ok(serde_json::Value::String(format!("[{}]", type_name)))
                     }
                 }
            };
            
            record.insert(col_name.to_string(), json_val.unwrap_or(serde_json::Value::Null));
        }
        data.push(serde_json::Value::Object(record));
    }

    let total_pages = if limit > 0 {
        if total % limit as u64 == 0 {
            total / limit as u64
        } else {
            total / limit as u64 + 1
        }
    } else {
        0
    };

    Ok(PaginationResult {
        data,
        total,
        page: req.page,
        page_size: req.page_size,
        total_pages: total_pages as u32,
    })
}

pub async fn execute_sql(
    conn: &SqlConnection,
    req: &ExecuteSqlRequest,
) -> Result<ExecuteSqlResponse, String> {
    let mut options = PgConnectOptions::new()
        .host(&conn.host)
        .port(conn.port)
        .username(&conn.username);

    if !req.database.is_empty() {
        options = options.database(&req.database);
    } else {
        options = options.database(&conn.database);
    }

    if let Some(pwd) = &conn.password {
        options = options.password(pwd);
    }

    options = options.log_statements(log::LevelFilter::Debug);

    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await
        .map_err(|e| format!("Failed to connect to PostgreSQL: {}", e))?;

    let start = Instant::now();
    let sql = req.sql.trim();
    let upper_sql = sql.to_uppercase();
    
    // Check if it's a SELECT query (or similar read query)
    let is_query = upper_sql.starts_with("SELECT") || upper_sql.starts_with("WITH") || upper_sql.starts_with("EXPLAIN") || upper_sql.starts_with("SHOW");

    if is_query {
        let mut stream = sqlx::query(sql).fetch(&pool);
        let mut rows = Vec::new();
        let limit = 2000;
        
        while let Some(row) = stream.try_next().await.map_err(|e| format!("Failed to fetch row: {}", e))? {
            rows.push(row);
            if rows.len() >= limit {
                break;
            }
        }
        
        let duration = start.elapsed();
        
        let mut columns = Vec::new();
        let mut data = Vec::new();

        if !rows.is_empty() {
            // Extract column names from the first row
            for col in rows[0].columns() {
                columns.push(col.name().to_string());
            }

            for row in rows {
                let mut record = serde_json::Map::new();
                for col in row.columns() {
                    let col_name = col.name();
                    let type_info = col.type_info();
                    let type_name = type_info.name();

                    let json_val = match type_name {
                        "BOOL" => row.try_get::<Option<bool>, _>(col_name).map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null)),
                        "INT2" | "INT4" | "INT8" => row.try_get::<Option<i64>, _>(col_name).map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null)),
                        "FLOAT4" | "FLOAT8" => row.try_get::<Option<f64>, _>(col_name).map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null)),
                        "VARCHAR" | "TEXT" | "CHAR" | "NAME" => row.try_get::<Option<String>, _>(col_name).map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null)),
                        "DATE" => row.try_get::<Option<chrono::NaiveDate>, _>(col_name).map(|v| v.map(|d| serde_json::Value::String(d.format("%Y-%m-%d").to_string())).unwrap_or(serde_json::Value::Null)),
                        "TIMESTAMP" => row.try_get::<Option<chrono::NaiveDateTime>, _>(col_name).map(|v| v.map(|dt| serde_json::Value::String(dt.format("%Y-%m-%d %H:%M:%S").to_string())).unwrap_or(serde_json::Value::Null)),
                        "TIMESTAMPTZ" => row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>(col_name).map(|v| v.map(|dt| serde_json::Value::String(dt.format("%Y-%m-%d %H:%M:%S").to_string())).unwrap_or(serde_json::Value::Null)),
                        "JSON" | "JSONB" => row.try_get::<Option<serde_json::Value>, _>(col_name).map(|v| v.unwrap_or(serde_json::Value::Null)),
                         _ => {
                             match row.try_get::<Option<String>, _>(col_name) {
                                 Ok(v) => Ok(serde_json::to_value(v).unwrap_or(serde_json::Value::Null)),
                                 Err(_) => Ok(serde_json::Value::String(format!("[{}]", type_name)))
                             }
                         }
                    };
                    
                    record.insert(col_name.to_string(), json_val.unwrap_or(serde_json::Value::Null));
                }
                data.push(serde_json::Value::Object(record));
            }
        }

        Ok(ExecuteSqlResponse {
            columns: Some(columns),
            rows: Some(data),
            affected_rows: None,
            execution_time_ms: duration.as_millis() as u64,
            message: Some(format!("Query executed successfully in {}ms", duration.as_millis())),
        })

    } else {
        // For non-query commands (INSERT, UPDATE, DELETE, etc.)
        let result = sqlx::query(sql)
            .execute(&pool)
            .await
            .map_err(|e| format!("Failed to execute command: {}", e))?;
        
        let duration = start.elapsed();
        let affected = result.rows_affected();

        Ok(ExecuteSqlResponse {
            columns: None,
            rows: None,
            affected_rows: Some(affected),
            execution_time_ms: duration.as_millis() as u64,
            message: Some(format!("Command executed successfully. Affected rows: {}", affected)),
        })
    }
}
