use super::models::{
    ApiEndpointBrief, ApiEndpointDetail, CreateEndpointPayload, UpdateEndpointPayload,
};
use actix_web::{HttpResponse, Responder, web};
use rusqlite::{Connection, params};

const DB_PATH: &str = "db/rsts.db";

// 数据模型已迁移至 models.rs

fn open_db() -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open(DB_PATH)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS api_endpoints (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            parent_id INTEGER,
            name TEXT NOT NULL,
            method TEXT NOT NULL,
            url TEXT NOT NULL,
            headers TEXT,
            params TEXT,
            body_type TEXT,
            body TEXT,
            content_type TEXT,
            order_index INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(parent_id) REFERENCES api_endpoints(id) ON DELETE SET NULL
        )",
        [],
    )?;
    Ok(conn)
}

pub async fn list_endpoints() -> impl Responder {
    match open_db() {
        Ok(conn) => {
            let mut stmt = match conn.prepare(
                "SELECT id, parent_id, name, method, url, order_index FROM api_endpoints ORDER BY parent_id IS NOT NULL, parent_id ASC, order_index ASC, id ASC",
            ) {
                Ok(s) => s,
                Err(e) => return HttpResponse::InternalServerError().body(format!("DB error: {}", e)),
            };
            let rows = stmt.query_map([], |row| {
                Ok(ApiEndpointBrief {
                    id: row.get(0)?,
                    parent_id: row.get::<_, Option<i64>>(1)?,
                    name: row.get(2)?,
                    method: row.get(3)?,
                    url: row.get(4)?,
                    order_index: row.get(5)?,
                })
            });
            match rows {
                Ok(iter) => {
                    let mut list: Vec<ApiEndpointBrief> = Vec::new();
                    for item in iter {
                        if let Ok(e) = item {
                            list.push(e);
                        }
                    }
                    HttpResponse::Ok().json(list)
                }
                Err(e) => HttpResponse::InternalServerError().body(format!("DB error: {}", e)),
            }
        }
        Err(e) => HttpResponse::InternalServerError().body(format!("DB open error: {}", e)),
    }
}

pub async fn get_endpoint(path: web::Path<i64>) -> impl Responder {
    let id = path.into_inner();
    match open_db() {
        Ok(conn) => {
            let mut stmt = match conn.prepare(
                "SELECT id, parent_id, name, method, url, headers, params, body_type, body, content_type, order_index, created_at, updated_at FROM api_endpoints WHERE id=?1",
            ) {
                Ok(s) => s,
                Err(e) => return HttpResponse::InternalServerError().body(format!("DB error: {}", e)),
            };
            match stmt.query_row(params![id], |row| {
                Ok(ApiEndpointDetail {
                    id: row.get(0)?,
                    parent_id: row.get(1)?,
                    name: row.get(2)?,
                    method: row.get(3)?,
                    url: row.get(4)?,
                    headers: row.get(5).ok(),
                    params: row.get(6).ok(),
                    body_type: row.get(7).ok(),
                    body: row.get(8).ok(),
                    content_type: row.get(9).ok(),
                    order_index: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            }) {
                Ok(detail) => HttpResponse::Ok().json(detail),
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    HttpResponse::NotFound().body("Not found")
                }
                Err(e) => HttpResponse::InternalServerError().body(format!("DB error: {}", e)),
            }
        }
        Err(e) => HttpResponse::InternalServerError().body(format!("DB open error: {}", e)),
    }
}

pub async fn create_endpoint(payload: web::Json<CreateEndpointPayload>) -> impl Responder {
    match open_db() {
        Ok(conn) => {
            let now = chrono::Utc::now().to_rfc3339();
            let order_index = payload.order_index.unwrap_or(0);
            match conn.execute(
                "INSERT INTO api_endpoints (parent_id, name, method, url, headers, params, body_type, body, content_type, order_index, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
                params![
                    payload.parent_id,
                    payload.name,
                    payload.method,
                    payload.url,
                    payload.headers.clone(),
                    payload.params.clone(),
                    payload.body_type.clone(),
                    payload.body.clone(),
                    payload.content_type.clone(),
                    order_index,
                    now
                ],
            ) {
                Ok(_) => {
                    let id = conn.last_insert_rowid();
                    HttpResponse::Ok().json(serde_json::json!({"id": id}))
                }
                Err(e) => HttpResponse::InternalServerError().body(format!("DB insert error: {}", e)),
            }
        }
        Err(e) => HttpResponse::InternalServerError().body(format!("DB open error: {}", e)),
    }
}

pub async fn update_endpoint(
    path: web::Path<i64>,
    payload: web::Json<UpdateEndpointPayload>,
) -> impl Responder {
    let id = path.into_inner();
    match open_db() {
        Ok(conn) => {
            let now = chrono::Utc::now().to_rfc3339();
            match conn.execute(
                "UPDATE api_endpoints SET parent_id=?1, name=?2, method=?3, url=?4, headers=?5, params=?6, body_type=?7, body=?8, content_type=?9, order_index=?10, updated_at=?11 WHERE id=?12",
                params![
                    payload.parent_id,
                    payload.name,
                    payload.method,
                    payload.url,
                    payload.headers.clone(),
                    payload.params.clone(),
                    payload.body_type.clone(),
                    payload.body.clone(),
                    payload.content_type.clone(),
                    payload.order_index.unwrap_or(0),
                    now,
                    id
                ],
            ) {
                Ok(rows) => {
                    if rows == 0 { HttpResponse::NotFound().body("Not found") } else { HttpResponse::Ok().body("OK") }
                }
                Err(e) => HttpResponse::InternalServerError().body(format!("DB update error: {}", e)),
            }
        }
        Err(e) => HttpResponse::InternalServerError().body(format!("DB open error: {}", e)),
    }
}

pub async fn delete_endpoint(path: web::Path<i64>) -> impl Responder {
    let id = path.into_inner();
    match open_db() {
        Ok(conn) => match conn.execute("DELETE FROM api_endpoints WHERE id=?1", params![id]) {
            Ok(rows) => {
                if rows == 0 {
                    HttpResponse::NotFound().body("Not found")
                } else {
                    HttpResponse::Ok().body("OK")
                }
            }
            Err(e) => HttpResponse::InternalServerError().body(format!("DB delete error: {}", e)),
        },
        Err(e) => HttpResponse::InternalServerError().body(format!("DB open error: {}", e)),
    }
}
