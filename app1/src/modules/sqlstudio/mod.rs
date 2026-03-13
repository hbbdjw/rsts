pub mod duckdb;
pub mod models;
pub mod mysql;
pub mod postgresql;
pub mod sqlite;

use self::models::{
    CreateConnectionRequest, MetadataRequest, SqlConnection, TestConnectionRequest,
    UpdateConnectionRequest, DeleteConnectionRequest,
};
use crate::modules::web::database::Database;
use actix_web::{HttpResponse, Responder, web};
use chrono::Utc;
use rusqlite::params;
use std::sync::Arc;

pub async fn test_connection_handler(req: web::Json<TestConnectionRequest>) -> impl Responder {
    match req.db_type.as_str() {
        "postgresql" => match postgresql::test_connection(&req).await {
            Ok(_) => HttpResponse::Ok()
                .json(serde_json::json!({ "code": 0, "msg": "Connection successful" })),
            Err(e) => HttpResponse::Ok().json(serde_json::json!({ "code": 1, "msg": e })),
        },
        "mysql" => HttpResponse::Ok()
            .json(serde_json::json!({ "code": 1, "msg": "MySQL not implemented yet" })),
        "sqlite3" => HttpResponse::Ok()
            .json(serde_json::json!({ "code": 1, "msg": "SQLite3 not implemented yet" })),
        "duckdb" => HttpResponse::Ok()
            .json(serde_json::json!({ "code": 1, "msg": "DuckDB not implemented yet" })),
        _ => HttpResponse::Ok()
            .json(serde_json::json!({ "code": 1, "msg": "Unsupported database type" })),
    }
}

pub async fn create_connection_handler(
    req: web::Json<CreateConnectionRequest>,
    db: web::Data<Arc<Database>>,
) -> impl Responder {
    let conn_arc = db.get_conn();
    let conn = conn_arc.lock().unwrap();
    let now = Utc::now().to_rfc3339();

    let result = conn.execute(
        "INSERT INTO sql_connections (name, db_type, host, port, username, password, database, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            req.name,
            req.db_type,
            req.host,
            req.port,
            req.username,
            req.password,
            req.database,
            now
        ],
    );

    match result {
        Ok(_) => {
            HttpResponse::Ok().json(serde_json::json!({ "code": 0, "msg": "Connection saved" }))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({ "code": 1, "msg": e.to_string() })),
    }
}

pub async fn update_connection_handler(
    req: web::Json<UpdateConnectionRequest>,
    db: web::Data<Arc<Database>>,
) -> impl Responder {
    let conn_arc = db.get_conn();
    let conn = conn_arc.lock().unwrap();
    let now = Utc::now().to_rfc3339();

    // 如果密码字段是 Some(val) 则更新，如果是 None 则保留原密码
    // 这里为了简单，如果 password 是 None 就不更新该字段
    // 或者前端传来空字符串表示不修改？
    // 通常做法：如果前端没传 password (Option::None)，则不更新该字段。
    // 但 rusqlite 的 execute 比较定长。
    // 我们可以先查询旧数据，或者动态构建 SQL。
    // 这里简化处理：假设 UpdateConnectionRequest 中 password 是 Option<String>
    // 如果是 Some(pwd) 且不为空，则更新；否则保留。
    
    // 为了防止覆写为空，先查一下旧密码比较稳妥，或者直接使用 COALESCE (如果支持)
    // SQLite COALESCE(?6, password) 可以实现 "如果参数是 NULL 则使用旧值"
    // 但 rust Option<String> 转 SQL 参数时，None 会变成 NULL。
    // 所以 SQL: password = COALESCE(?6, password)

    let result = conn.execute(
        "UPDATE sql_connections SET name=?1, db_type=?2, host=?3, port=?4, username=?5, 
         password=COALESCE(?6, password), database=?7, updated_at=?8 WHERE id=?9",
        params![
            req.name,
            req.db_type,
            req.host,
            req.port,
            req.username,
            req.password,
            req.database,
            now,
            req.id
        ],
    );

    match result {
        Ok(rows) => {
            if rows > 0 {
                HttpResponse::Ok().json(serde_json::json!({ "code": 0, "msg": "Connection updated" }))
            } else {
                 HttpResponse::Ok().json(serde_json::json!({ "code": 1, "msg": "Connection not found" }))
            }
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({ "code": 1, "msg": e.to_string() })),
    }
}

pub async fn delete_connection_handler(
    req: web::Json<DeleteConnectionRequest>,
    db: web::Data<Arc<Database>>,
) -> impl Responder {
    let conn_arc = db.get_conn();
    let conn = conn_arc.lock().unwrap();

    let result = conn.execute(
        "DELETE FROM sql_connections WHERE id=?1",
        params![req.id],
    );

    match result {
        Ok(rows) => {
             if rows > 0 {
                HttpResponse::Ok().json(serde_json::json!({ "code": 0, "msg": "Connection deleted" }))
            } else {
                 HttpResponse::Ok().json(serde_json::json!({ "code": 1, "msg": "Connection not found" }))
            }
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({ "code": 1, "msg": e.to_string() })),
    }
}

pub async fn list_connections_handler(db: web::Data<Arc<Database>>) -> impl Responder {
    let conn_arc = db.get_conn();
    let conn = conn_arc.lock().unwrap();
    let stmt_result = conn.prepare(
        "SELECT id, name, db_type, host, port, username, database, created_at FROM sql_connections",
    );

    match stmt_result {
        Ok(mut stmt) => {
            let connections_iter = stmt.query_map([], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "db_type": row.get::<_, String>(2)?,
                    "host": row.get::<_, String>(3)?,
                    "port": row.get::<_, u16>(4)?,
                    "username": row.get::<_, String>(5)?,
                    "database": row.get::<_, String>(6)?,
                    "created_at": row.get::<_, String>(7)?,
                }))
            });

            match connections_iter {
                Ok(iter) => {
                    let connections: Vec<_> = iter.filter_map(Result::ok).collect();
                    HttpResponse::Ok().json(serde_json::json!({ "code": 0, "data": connections }))
                }
                Err(e) => HttpResponse::InternalServerError()
                    .json(serde_json::json!({ "code": 1, "msg": e.to_string() })),
            }
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({ "code": 1, "msg": e.to_string() })),
    }
}

pub async fn get_metadata_handler(
    req: web::Json<MetadataRequest>,
    db: web::Data<Arc<Database>>,
) -> impl Responder {
    let conn_arc = db.get_conn();
    let conn = conn_arc.lock().unwrap();
    // Fetch connection details from SQLite
    let conn_query = conn.query_row(
        "SELECT id, name, db_type, host, port, username, password, database FROM sql_connections WHERE id = ?1",
        params![req.connection_id],
        |row| {
            Ok(SqlConnection {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                db_type: row.get(2)?,
                host: row.get(3)?,
                port: row.get(4)?,
                username: row.get(5)?,
                password: row.get(6)?,
                database: row.get(7)?,
                created_at: None,
                updated_at: None,
            })
        }
    );

    match conn_query {
        Ok(sql_conn) => match sql_conn.db_type.as_str() {
            "postgresql" => {
                match postgresql::get_metadata(
                    &sql_conn,
                    &req.action,
                    req.database.as_deref(),
                    req.schema.as_deref(),
                )
                .await
                {
                    Ok(data) => {
                        HttpResponse::Ok().json(serde_json::json!({ "code": 0, "data": data }))
                    }
                    Err(e) => HttpResponse::Ok().json(serde_json::json!({ "code": 1, "msg": e })),
                }
            }
            _ => HttpResponse::Ok()
                .json(serde_json::json!({ "code": 1, "msg": "Database type not supported yet" })),
        },
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            HttpResponse::Ok().json(serde_json::json!({ "code": 1, "msg": "Connection not found" }))
        }
        Err(e) => HttpResponse::InternalServerError()
            .json(serde_json::json!({ "code": 1, "msg": e.to_string() })),
    }
}
