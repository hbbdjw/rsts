use super::models::{
    AddColumnPayload, ColumnInfo, CreateTablePayload, DatabaseInfo, DropColumnPayload,
    DropTablePayload, PaginationResult, QueryParams, RenameColumnPayload, RenameTablePayload,
    RowBatchDeletePayload, RowDeletePayload, RowInsertPayload, RowUpdatePayload, SqlQueryPayload,
    TableInfo,
};
use actix_web::{HttpRequest, HttpResponse, Responder, web};
use log::error as log_error;
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, Result as RusqliteResult};
use std::fs;
use std::path::Path;

// 数据模型已迁移至 models.rs

// 查询db目录下所有数据库文件
pub async fn get_all_databases() -> impl Responder {
    let db_dir = Path::new("db");
    let mut databases = Vec::new();

    if let Ok(entries) = fs::read_dir(db_dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    let path = entry.path();
                    if let Some(ext) = path.extension() {
                        if ext == "db" {
                            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                                databases.push(DatabaseInfo {
                                    name: file_name.to_string(),
                                    path: path.to_str().unwrap_or("").to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    HttpResponse::Ok().json(databases)
}

// 根据数据库名称查询表信息
pub async fn get_tables_by_database(
    _req: HttpRequest,
    params: web::Query<QueryParams>,
) -> impl Responder {
    let db_name = &params.db_name;
    let db_path = format!("db/{}", db_name);

    if !Path::new(&db_path).exists() {
        return HttpResponse::NotFound().body(format!("Database {} not found", db_name));
    }

    match get_tables_info(&db_path) {
        Ok(tables) => HttpResponse::Ok().json(tables),
        Err(e) => {
            log_error!("Failed to get tables: {}", e);
            HttpResponse::InternalServerError().body(format!("Error: {}", e))
        }
    }
}

// 查询表数据（支持分页）
pub async fn get_table_data(_req: HttpRequest, params: web::Query<QueryParams>) -> impl Responder {
    let db_name = &params.db_name;
    let table_name = match &params.table_name {
        Some(name) => name,
        None => return HttpResponse::BadRequest().body("Table name is required"),
    };

    let page = params.page.unwrap_or(1);
    let page_size = params.page_size.unwrap_or(10);
    let db_path = format!("db/{}", db_name);

    if !Path::new(&db_path).exists() {
        return HttpResponse::NotFound().body(format!("Database {} not found", db_name));
    }

    match get_table_content(&db_path, table_name, page, page_size) {
        Ok(result) => HttpResponse::Ok().json(result),
        Err(e) => {
            log_error!("Failed to get table data: {}", e);
            HttpResponse::InternalServerError().body(format!("Error: {}", e))
        }
    }
}

// 内部函数：获取数据库中的表信息
fn get_tables_info(db_path: &str) -> RusqliteResult<Vec<TableInfo>> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?;

    let table_iter = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut tables = Vec::new();

    for table_name in table_iter.filter_map(Result::ok) {
        let table_name = table_name;
        let columns = get_columns_info(&conn, &table_name)?;

        tables.push(TableInfo {
            name: table_name.to_string(),
            columns,
        });
    }

    Ok(tables)
}

// 内部函数：获取表的列信息
fn get_columns_info(conn: &Connection, table_name: &str) -> RusqliteResult<Vec<ColumnInfo>> {
    let sql = format!("PRAGMA table_info({})", table_name);
    let mut stmt = conn.prepare(&sql)?;

    let column_iter = stmt.query_map([], |row| {
        Ok(ColumnInfo {
            name: row.get(1)?,
            data_type: row.get(2)?,
            not_null: row.get(3)?,
            primary_key: row.get(5)?,
        })
    })?;

    let mut columns = Vec::new();
    for column in column_iter {
        columns.push(column?);
    }

    Ok(columns)
}

// 内部函数：获取表内容（支持分页）
fn get_table_content(
    db_path: &str,
    table_name: &str,
    page: u32,
    page_size: u32,
) -> RusqliteResult<PaginationResult> {
    let conn = Connection::open(db_path)?;

    // 检查表是否存在
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
        [table_name],
        |row| row.get(0),
    )?;

    if !exists {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }

    // 获取总记录数
    let total: u64 =
        conn.query_row(&format!("SELECT COUNT(*) FROM {}", table_name), [], |row| {
            let count: i64 = row.get(0)?;
            Ok(count as u64)
        })?;

    // 计算总页数
    let total_pages = if total % page_size as u64 == 0 {
        total / page_size as u64
    } else {
        total / page_size as u64 + 1
    };

    // 计算偏移量
    let offset = (page.saturating_sub(1) * page_size) as u64;

    // 获取列名
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table_name))?;
    let column_iter = stmt.query_map([], |row| row.get::<_, String>(1))?;
    let columns: Vec<String> = column_iter
        .map(|r| r.unwrap_or("unknown".to_string()))
        .collect();

    // 查询分页数据
    let sql = format!(
        "SELECT * FROM {} LIMIT {} OFFSET {}",
        table_name, page_size, offset
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;

    let mut data = Vec::new();

    while let Some(row) = rows.next()? {
        let mut record = serde_json::Map::new();

        for (i, column) in columns.iter().enumerate() {
            // 尝试获取不同类型的值
            if let Ok(value) = row.get::<_, i64>(i) {
                record.insert(
                    column.clone(),
                    serde_json::Value::Number(serde_json::Number::from(value)),
                );
            } else if let Ok(value) = row.get::<_, f64>(i) {
                if value.fract() == 0.0 {
                    // 如果是整数，转换为整数类型
                    record.insert(
                        column.clone(),
                        serde_json::Value::Number(serde_json::Number::from(value as i64)),
                    );
                } else {
                    // 如果是小数，转换为浮点数类型
                    record.insert(
                        column.clone(),
                        serde_json::Value::Number(
                            serde_json::Number::from_f64(value)
                                .unwrap_or_else(|| serde_json::Number::from(0)),
                        ),
                    );
                }
            } else if let Ok(value) = row.get::<_, bool>(i) {
                record.insert(column.clone(), serde_json::Value::Bool(value));
            } else if let Ok(value) = row.get::<_, String>(i) {
                record.insert(column.clone(), serde_json::Value::String(value));
            } else {
                // 如果无法识别类型，设为null
                record.insert(column.clone(), serde_json::Value::Null);
            }
        }

        data.push(serde_json::Value::Object(record));
    }

    Ok(PaginationResult {
        data,
        total,
        page,
        page_size,
        total_pages: total_pages as u32,
    })
}

// -------- 表与列操作实现 --------
pub async fn create_table(payload: web::Json<CreateTablePayload>) -> impl Responder {
    let db_path = format!("db/{}", payload.db_name);
    if !Path::new(&db_path).exists() {
        if let Some(parent) = Path::new(&db_path).parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                return HttpResponse::InternalServerError()
                    .body(format!("Create db dir error: {}", e));
            }
        }
    }

    // 构建CREATE TABLE语句
    if payload.columns.is_empty() {
        return HttpResponse::BadRequest().body("columns cannot be empty");
    }
    let mut cols_sql: Vec<String> = Vec::new();
    for c in &payload.columns {
        let mut part = format!("{} {}", c.name, c.data_type);
        if c.not_null.unwrap_or(false) {
            part.push_str(" NOT NULL");
        }
        if c.primary_key.unwrap_or(false) {
            part.push_str(" PRIMARY KEY");
        }
        cols_sql.push(part);
    }
    let sql = format!(
        "CREATE TABLE IF NOT EXISTS {} ({})",
        payload.table_name,
        cols_sql.join(", ")
    );

    match Connection::open(&db_path).and_then(|conn| conn.execute(&sql, []).map(|_| ())) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({"status":"ok"})),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {}", e)),
    }
}

pub async fn drop_table(payload: web::Json<DropTablePayload>) -> impl Responder {
    let db_path = format!("db/{}", payload.db_name);
    if !Path::new(&db_path).exists() {
        return HttpResponse::NotFound().body(format!("Database {} not found", payload.db_name));
    }
    let sql = format!("DROP TABLE IF EXISTS {}", payload.table_name);
    match Connection::open(&db_path).and_then(|conn| conn.execute(&sql, []).map(|_| ())) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({"status":"ok"})),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {}", e)),
    }
}

pub async fn rename_table(payload: web::Json<RenameTablePayload>) -> impl Responder {
    let db_path = format!("db/{}", payload.db_name);
    if !Path::new(&db_path).exists() {
        return HttpResponse::NotFound().body(format!("Database {} not found", payload.db_name));
    }
    let sql = format!(
        "ALTER TABLE {} RENAME TO {}",
        payload.table_name, payload.new_name
    );
    match Connection::open(&db_path).and_then(|conn| conn.execute(&sql, []).map(|_| ())) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({"status":"ok"})),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {}", e)),
    }
}

pub async fn rename_column(payload: web::Json<RenameColumnPayload>) -> impl Responder {
    let db_path = format!("db/{}", payload.db_name);
    if !Path::new(&db_path).exists() {
        return HttpResponse::NotFound().body(format!("Database {} not found", payload.db_name));
    }
    let sql = format!(
        "ALTER TABLE {} RENAME COLUMN {} TO {}",
        payload.table_name, payload.old_name, payload.new_name
    );
    match Connection::open(&db_path).and_then(|conn| conn.execute(&sql, []).map(|_| ())) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({"status":"ok"})),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {}", e)),
    }
}

pub async fn add_column(payload: web::Json<AddColumnPayload>) -> impl Responder {
    let db_path = format!("db/{}", payload.db_name);
    if !Path::new(&db_path).exists() {
        return HttpResponse::NotFound().body(format!("Database {} not found", payload.db_name));
    }
    let mut sql = format!(
        "ALTER TABLE {} ADD COLUMN {} {}",
        payload.table_name, payload.name, payload.data_type
    );
    if payload.not_null.unwrap_or(false) {
        sql.push_str(" NOT NULL");
    }
    if let Some(def) = &payload.default {
        sql.push_str(&format!(" DEFAULT {}", json_value_to_sql_literal(def)));
    }
    match Connection::open(&db_path).and_then(|conn| conn.execute(&sql, []).map(|_| ())) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({"status":"ok"})),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {}", e)),
    }
}

pub async fn drop_column(payload: web::Json<DropColumnPayload>) -> impl Responder {
    let db_path = format!("db/{}", payload.db_name);
    if !Path::new(&db_path).exists() {
        return HttpResponse::NotFound().body(format!("Database {} not found", payload.db_name));
    }
    let sql = format!(
        "ALTER TABLE {} DROP COLUMN {}",
        payload.table_name, payload.column_name
    );
    match Connection::open(&db_path).and_then(|conn| conn.execute(&sql, []).map(|_| ())) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({"status":"ok"})),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {}", e)),
    }
}

// -------- 行操作与SQL控制台实现 --------
pub async fn insert_row(payload: web::Json<RowInsertPayload>) -> impl Responder {
    let db_path = format!("db/{}", payload.db_name);
    if !Path::new(&db_path).exists() {
        return HttpResponse::NotFound().body(format!("Database {} not found", payload.db_name));
    }
    if payload.values.is_empty() {
        return HttpResponse::BadRequest().body("values cannot be empty");
    }
    let cols: Vec<String> = payload.values.keys().cloned().collect();
    let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("?{}", i)).collect();
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        payload.table_name,
        cols.join(","),
        placeholders.join(",")
    );

    let mut params_vec: Vec<SqlValue> = Vec::new();
    for v in payload.values.values() {
        params_vec.push(json_value_to_sql_value(v));
    }

    match Connection::open(&db_path).and_then(|conn| {
        conn.execute(&sql, rusqlite::params_from_iter(params_vec))
            .map(|_| ())
    }) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({"status":"ok"})),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {}", e)),
    }
}

pub async fn update_row(payload: web::Json<RowUpdatePayload>) -> impl Responder {
    let db_path = format!("db/{}", payload.db_name);
    if !Path::new(&db_path).exists() {
        return HttpResponse::NotFound().body(format!("Database {} not found", payload.db_name));
    }
    if payload.values.is_empty() {
        return HttpResponse::BadRequest().body("values cannot be empty");
    }
    let sets: Vec<String> = payload
        .values
        .keys()
        .map(|k| format!("{} = ?", k))
        .collect();
    let sql = format!(
        "UPDATE {} SET {} WHERE {} = ?",
        payload.table_name,
        sets.join(", "),
        payload.pk_column
    );

    let mut params_vec: Vec<SqlValue> = payload
        .values
        .values()
        .map(|v| json_value_to_sql_value(v))
        .collect();
    params_vec.push(json_value_to_sql_value(&payload.pk_value));

    match Connection::open(&db_path).and_then(|conn| {
        conn.execute(&sql, rusqlite::params_from_iter(params_vec))
            .map(|_| ())
    }) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({"status":"ok"})),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {}", e)),
    }
}

pub async fn delete_row(payload: web::Json<RowDeletePayload>) -> impl Responder {
    let db_path = format!("db/{}", payload.db_name);
    if !Path::new(&db_path).exists() {
        return HttpResponse::NotFound().body(format!("Database {} not found", payload.db_name));
    }
    let sql = format!(
        "DELETE FROM {} WHERE {} = ?",
        payload.table_name, payload.pk_column
    );
    let pk_val = json_value_to_sql_value(&payload.pk_value);
    match Connection::open(&db_path)
        .and_then(|conn| conn.execute(&sql, rusqlite::params![pk_val]).map(|_| ()))
    {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({"status":"ok"})),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {}", e)),
    }
}

pub async fn batch_delete_rows(payload: web::Json<RowBatchDeletePayload>) -> impl Responder {
    let db_path = format!("db/{}", payload.db_name);
    if !Path::new(&db_path).exists() {
        return HttpResponse::NotFound().body(format!("Database {} not found", payload.db_name));
    }
    if payload.pk_values.is_empty() {
        return HttpResponse::BadRequest().body("pk_values cannot be empty");
    }
    let placeholders: Vec<String> = (1..=payload.pk_values.len())
        .map(|i| format!("?{}", i))
        .collect();
    let sql = format!(
        "DELETE FROM {} WHERE {} IN ({})",
        payload.table_name,
        payload.pk_column,
        placeholders.join(",")
    );
    let params_vec: Vec<SqlValue> = payload
        .pk_values
        .iter()
        .map(|v| json_value_to_sql_value(v))
        .collect();
    match Connection::open(&db_path).and_then(|conn| {
        conn.execute(&sql, rusqlite::params_from_iter(params_vec))
            .map(|_| ())
    }) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({"status":"ok"})),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {}", e)),
    }
}

pub async fn sql_query(payload: web::Json<SqlQueryPayload>) -> impl Responder {
    let db_path = format!("db/{}", payload.db_name);
    if !Path::new(&db_path).exists() {
        return HttpResponse::NotFound().body(format!("Database {} not found", payload.db_name));
    }
    let conn = match Connection::open(&db_path) {
        Ok(c) => c,
        Err(e) => return HttpResponse::InternalServerError().body(format!("Error: {}", e)),
    };
    let sql_trimmed = payload.sql.trim().to_uppercase();
    let params_vec: Vec<SqlValue> = payload
        .params
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|v| json_value_to_sql_value(&v))
        .collect();

    if sql_trimmed.starts_with("SELECT") {
        let mut stmt = match conn.prepare(&payload.sql) {
            Ok(s) => s,
            Err(e) => return HttpResponse::BadRequest().body(format!("SQL prepare error: {}", e)),
        };
        // 先获取列名，避免与rows的可变借用冲突
        let col_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        let mut rows = match stmt.query(rusqlite::params_from_iter(params_vec.clone())) {
            Ok(r) => r,
            Err(e) => return HttpResponse::BadRequest().body(format!("SQL query error: {}", e)),
        };
        let mut data = Vec::new();
        while let Some(row) = match rows.next() {
            Ok(r) => r,
            Err(e) => return HttpResponse::BadRequest().body(format!("Row fetch error: {}", e)),
        } {
            let mut record = serde_json::Map::new();
            for (i, col) in col_names.iter().enumerate() {
                let v = row.get_ref(i);
                let json_v = match v {
                    Ok(rusqlite::types::ValueRef::Null) => serde_json::Value::Null,
                    Ok(rusqlite::types::ValueRef::Integer(i)) => {
                        serde_json::Value::Number(serde_json::Number::from(i))
                    }
                    Ok(rusqlite::types::ValueRef::Real(f)) => serde_json::Number::from_f64(f)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null),
                    Ok(rusqlite::types::ValueRef::Text(t)) => {
                        serde_json::Value::String(String::from_utf8_lossy(t).to_string())
                    }
                    Ok(rusqlite::types::ValueRef::Blob(b)) => {
                        serde_json::Value::String(format!("<{} bytes>", b.len()))
                    }
                    Err(_) => serde_json::Value::Null,
                };
                record.insert(col.clone(), json_v);
            }
            data.push(serde_json::Value::Object(record));
        }
        HttpResponse::Ok().json(serde_json::json!({"status":"ok","data":data}))
    } else {
        match conn.execute(&payload.sql, rusqlite::params_from_iter(params_vec)) {
            Ok(changed) => {
                HttpResponse::Ok().json(serde_json::json!({"status":"ok","changed":changed}))
            }
            Err(e) => HttpResponse::BadRequest().body(format!("SQL exec error: {}", e)),
        }
    }
}

// -------- 帮助函数：JSON值到SQL字面量/参数 --------
fn json_value_to_sql_literal(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::Bool(b) => {
            if *b {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("'{}'", s.replace("'", "''")),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => "NULL".to_string(),
    }
}

fn json_value_to_sql_value(v: &serde_json::Value) -> SqlValue {
    match v {
        serde_json::Value::Null => SqlValue::Null,
        serde_json::Value::Bool(b) => SqlValue::Integer(if *b { 1 } else { 0 }),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                SqlValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                SqlValue::Real(f)
            } else {
                SqlValue::Text(n.to_string())
            }
        }
        serde_json::Value::String(s) => SqlValue::Text(s.clone()),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => SqlValue::Null,
    }
}
