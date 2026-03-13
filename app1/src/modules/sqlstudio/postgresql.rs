use super::models::{MetadataResponse, SqlConnection, TestConnectionRequest};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, Row};
use std::time::Duration;

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
