use actix_web::{HttpResponse, Responder, web};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::json;
use std::path::Path;

use crate::log_info;

const DB_PATH: &str = "db/rsts.db";
use super::models::{SshGroup, SshGroupInput, SshServer, SshServerInput};

// 数据模型已迁移至 models.rs

fn ensure_group_table(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ssh_groups (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            is_default INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )?;
    Ok(())
}

fn ensure_server_table(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ssh_servers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            alias TEXT NOT NULL,
            hostname TEXT NOT NULL,
            port INTEGER NOT NULL DEFAULT 22,
            username TEXT NOT NULL,
            password TEXT,
            group_id INTEGER,
            remark TEXT
        )",
        [],
    )?;
    Ok(())
}

fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    column_type: &str,
    default: Option<&str>,
) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    if columns.iter().any(|c| c == column) {
        return Ok(());
    }
    let default_sql = default
        .map(|d| format!(" DEFAULT {}", d))
        .unwrap_or_default();
    conn.execute(
        &format!(
            "ALTER TABLE {} ADD COLUMN {} {}{}",
            table, column, column_type, default_sql
        ),
        [],
    )?;
    Ok(())
}

fn ensure_default_group(conn: &Connection) -> rusqlite::Result<i64> {
    let id: Option<i64> = conn
        .query_row(
            "SELECT id FROM ssh_groups WHERE is_default=1 LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(id) = id {
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO ssh_groups (name, is_default) VALUES (?1, 1)",
        params!["默认分组"],
    )?;
    Ok(conn.last_insert_rowid())
}

fn open_db() -> rusqlite::Result<Connection> {
    if !Path::new(DB_PATH).exists() {
        std::fs::create_dir_all("db").ok();
        std::fs::File::create(DB_PATH).ok();
    }
    let conn = Connection::open(DB_PATH)?;
    ensure_group_table(&conn)?;
    ensure_server_table(&conn)?;
    ensure_column(&conn, "ssh_servers", "group_id", "INTEGER", None)?;
    ensure_column(&conn, "ssh_servers", "remark", "TEXT", None)?;
    ensure_default_group(&conn)?;
    Ok(conn)
}

fn ok<T: serde::Serialize>(data: T) -> HttpResponse {
    HttpResponse::Ok().json(json!({
        "code": "0000",
        "msg": "success",
        "data": data
    }))
}

fn err(msg: String) -> HttpResponse {
    HttpResponse::Ok().json(json!({
        "code": "5000",
        "msg": msg,
        "data": null
    }))
}

pub async fn list_groups() -> impl Responder {
    match open_db() {
        Ok(conn) => {
            let mut stmt = match conn.prepare(
                "SELECT id, name, is_default FROM ssh_groups ORDER BY is_default DESC, id ASC",
            ) {
                Ok(s) => s,
                Err(e) => return err(format!("DB error: {}", e)),
            };
            let rows = stmt.query_map([], |row| {
                Ok(SshGroup {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    is_default: row.get(2)?,
                })
            });
            match rows {
                Ok(iter) => {
                    let mut groups: Vec<SshGroup> = Vec::new();
                    for item in iter {
                        if let Ok(g) = item {
                            groups.push(g);
                        }
                    }
                    ok(groups)
                }
                Err(e) => err(format!("DB error: {}", e)),
            }
        }
        Err(e) => err(format!("DB open error: {}", e)),
    }
}

pub async fn create_group(payload: web::Json<SshGroupInput>) -> impl Responder {
    match open_db() {
        Ok(conn) => {
            match conn.execute(
                "INSERT INTO ssh_groups (name, is_default) VALUES (?1, 0)",
                params![payload.name.clone()],
            ) {
                Ok(_) => {
                    let id = conn.last_insert_rowid();
                    ok(SshGroup {
                        id,
                        name: payload.name.clone(),
                        is_default: 0,
                    })
                }
                Err(e) => err(format!("DB insert error: {}", e)),
            }
        }
        Err(e) => err(format!("DB open error: {}", e)),
    }
}

pub async fn update_group(
    path: web::Path<i64>,
    payload: web::Json<SshGroupInput>,
) -> impl Responder {
    let id = path.into_inner();
    match open_db() {
        Ok(conn) => {
            match conn.execute(
                "UPDATE ssh_groups SET name=?1 WHERE id=?2",
                params![payload.name.clone(), id],
            ) {
                Ok(rows) => {
                    if rows == 0 {
                        return err("Not found".to_string());
                    }
                    let is_default: i32 = conn
                        .query_row(
                            "SELECT is_default FROM ssh_groups WHERE id=?1",
                            params![id],
                            |row| row.get(0),
                        )
                        .unwrap_or(0);
                    ok(SshGroup {
                        id,
                        name: payload.name.clone(),
                        is_default,
                    })
                }
                Err(e) => err(format!("DB update error: {}", e)),
            }
        }
        Err(e) => err(format!("DB open error: {}", e)),
    }
}

pub async fn delete_group(path: web::Path<i64>) -> impl Responder {
    let id = path.into_inner();
    match open_db() {
        Ok(conn) => {
            let is_default: Option<i32> = conn
                .query_row(
                    "SELECT is_default FROM ssh_groups WHERE id=?1",
                    params![id],
                    |row| row.get(0),
                )
                .optional()
                .unwrap_or(None);
            if is_default.unwrap_or(0) == 1 {
                return err("Default group cannot be deleted".to_string());
            }
            let default_id = match ensure_default_group(&conn) {
                Ok(v) => v,
                Err(e) => return err(format!("DB error: {}", e)),
            };
            let _ = conn.execute(
                "UPDATE ssh_servers SET group_id=?1 WHERE group_id=?2",
                params![default_id, id],
            );
            match conn.execute("DELETE FROM ssh_groups WHERE id=?1", params![id]) {
                Ok(rows) => {
                    if rows == 0 {
                        err("Not found".to_string())
                    } else {
                        ok(json!({"id": id}))
                    }
                }
                Err(e) => err(format!("DB delete error: {}", e)),
            }
        }
        Err(e) => err(format!("DB open error: {}", e)),
    }
}

pub async fn list_servers() -> impl Responder {
    match open_db() {
        Ok(conn) => {
            let default_group_id = match ensure_default_group(&conn) {
                Ok(v) => v,
                Err(e) => return err(format!("DB error: {}", e)),
            };
            let mut stmt = match conn.prepare(
                "SELECT id, alias, hostname, port, username, password, group_id, remark FROM ssh_servers ORDER BY alias ASC, id ASC",
            ) {
                Ok(s) => s,
                Err(e) => return err(format!("DB error: {}", e)),
            };
            let rows = stmt.query_map([], |row| {
                Ok(SshServer {
                    id: row.get(0)?,
                    alias: row.get(1)?,
                    hostname: row.get(2)?,
                    port: row.get(3)?,
                    username: row.get(4)?,
                    password: row.get(5).ok(),
                    group_id: row.get::<_, Option<i64>>(6)?.unwrap_or(default_group_id),
                    remark: row.get(7).ok(),
                })
            });
            match rows {
                Ok(iter) => {
                    let mut servers: Vec<SshServer> = Vec::new();
                    for item in iter {
                        if let Ok(s) = item {
                            servers.push(s);
                        }
                    }
                    ok(servers)
                }
                Err(e) => err(format!("DB error: {}", e)),
            }
        }
        Err(e) => err(format!("DB open error: {}", e)),
    }
}

pub async fn create_server(payload: web::Json<SshServerInput>) -> impl Responder {
    match open_db() {
        Ok(conn) => {
            let port = payload.port.unwrap_or(22);
            let default_group_id = match ensure_default_group(&conn) {
                Ok(v) => v,
                Err(e) => return err(format!("DB error: {}", e)),
            };
            let group_id = payload.group_id.unwrap_or(default_group_id);
            let alias = payload.alias.clone().unwrap_or_default();
            match conn.execute(
                "INSERT INTO ssh_servers (alias, hostname, port, username, password, group_id, remark) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![alias, payload.hostname, port, payload.username, payload.password.clone(), group_id, payload.remark.clone()],
            ) {
                Ok(_) => {
                    let id = conn.last_insert_rowid();
                    let server = SshServer {
                        id,
                        alias,
                        hostname: payload.hostname.clone(),
                        port,
                        username: payload.username.clone(),
                        password: payload.password.clone(),
                        group_id,
                        remark: payload.remark.clone(),
                    };
                    log_info!("Created ssh server config: {}@{}:{} ({})", server.username, server.hostname, server.port, server.alias);
                    ok(server)
                }
                Err(e) => err(format!("DB insert error: {}", e)),
            }
        }
        Err(e) => err(format!("DB open error: {}", e)),
    }
}

pub async fn update_server(
    path: web::Path<i64>,
    payload: web::Json<SshServerInput>,
) -> impl Responder {
    let id = path.into_inner();
    match open_db() {
        Ok(conn) => {
            let port = payload.port.unwrap_or(22);
            let default_group_id = match ensure_default_group(&conn) {
                Ok(v) => v,
                Err(e) => return err(format!("DB error: {}", e)),
            };
            let group_id = payload.group_id.unwrap_or(default_group_id);
            let alias = payload.alias.clone().unwrap_or_default();
            match conn.execute(
                "UPDATE ssh_servers SET alias=?1, hostname=?2, port=?3, username=?4, password=?5, group_id=?6, remark=?7 WHERE id=?8",
                params![alias, payload.hostname, port, payload.username, payload.password.clone(), group_id, payload.remark.clone(), id],
            ) {
                Ok(rows) => {
                    if rows == 0 {
                        return err("Not found".to_string());
                    }
                    let server = SshServer {
                        id,
                        alias,
                        hostname: payload.hostname.clone(),
                        port,
                        username: payload.username.clone(),
                        password: payload.password.clone(),
                        group_id,
                        remark: payload.remark.clone(),
                    };
                    ok(server)
                }
                Err(e) => err(format!("DB update error: {}", e)),
            }
        }
        Err(e) => err(format!("DB open error: {}", e)),
    }
}

pub async fn delete_server(path: web::Path<i64>) -> impl Responder {
    let id = path.into_inner();
    match open_db() {
        Ok(conn) => match conn.execute("DELETE FROM ssh_servers WHERE id=?1", params![id]) {
            Ok(rows) => {
                if rows == 0 {
                    err("Not found".to_string())
                } else {
                    ok(json!({"id": id}))
                }
            }
            Err(e) => err(format!("DB delete error: {}", e)),
        },
        Err(e) => err(format!("DB open error: {}", e)),
    }
}
