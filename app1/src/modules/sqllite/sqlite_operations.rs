use rusqlite::{Connection, Result, params};
// use std::fs;
use std::path::Path;

// 导入日志宏
// use crate::log_debug;
// use crate::log_error;
use crate::log_info;
use crate::log_warn;

// 定义用户数据结构
#[derive(Debug)]
#[allow(dead_code)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub age: i32,
}

// 初始化数据库连接
fn init_db(db_path: &str) -> Result<Connection> {
    // 检查数据库文件是否存在，不存在则创建
    let db_exists = Path::new(db_path).exists();

    // 建立数据库连接
    let conn = Connection::open(db_path)?;

    // 如果是新创建的数据库，创建表
    if !db_exists {
        create_users_table(&conn)?;
        log_info!("数据库已创建，表结构已初始化。");
    } else {
        log_info!("成功连接到现有数据库。");
    }

    Ok(conn)
}

// 创建用户表
fn create_users_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            email TEXT NOT NULL UNIQUE,
            age INTEGER NOT NULL
        )",
        [],
    )?;
    log_info!("users表创建成功。");
    Ok(())
}

// 插入新用户
fn add_user(conn: &Connection, name: &str, email: &str, age: i32) -> Result<()> {
    conn.execute(
        "INSERT INTO users (name, email, age) VALUES (?1, ?2, ?3)",
        params![name, email, age],
    )?;
    log_info!("用户添加成功：姓名={}, 邮箱={}, 年龄={}", name, email, age);
    Ok(())
}

// 查询所有用户
fn get_all_users(conn: &Connection) -> Result<Vec<User>> {
    let mut stmt = conn.prepare("SELECT id, name, email, age FROM users")?;
    let user_iter = stmt.query_map([], |row| {
        Ok(User {
            id: row.get(0)?,
            name: row.get(1)?,
            email: row.get(2)?,
            age: row.get(3)?,
        })
    })?;

    let mut users = Vec::new();
    for user in user_iter {
        users.push(user?);
    }

    Ok(users)
}

// 根据ID查询用户
#[allow(dead_code)]
fn get_user_by_id(conn: &Connection, id: i32) -> Result<Option<User>> {
    let mut stmt = conn.prepare("SELECT id, name, email, age FROM users WHERE id = ?1")?;
    let user = stmt.query_row(params![id], |row| {
        Ok(User {
            id: row.get(0)?,
            name: row.get(1)?,
            email: row.get(2)?,
            age: row.get(3)?,
        })
    });

    match user {
        Ok(user) => Ok(Some(user)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

// 更新用户信息
#[allow(dead_code)]
fn update_user(conn: &Connection, id: i32, name: &str, email: &str, age: i32) -> Result<bool> {
    let rows_affected = conn.execute(
        "UPDATE users SET name = ?1, email = ?2, age = ?3 WHERE id = ?4",
        params![name, email, age, id],
    )?;

    if rows_affected > 0 {
        log_info!(
            "用户更新成功：ID={}, 新姓名={}, 新邮箱={}, 新年龄={}",
            id,
            name,
            email,
            age
        );
        Ok(true)
    } else {
        log_warn!("未找到ID为{}的用户。", id);
        Ok(false)
    }
}

// 删除用户
#[allow(dead_code)]
fn delete_user(conn: &Connection, id: i32) -> Result<bool> {
    let rows_affected = conn.execute("DELETE FROM users WHERE id = ?1", params![id])?;

    if rows_affected > 0 {
        log_info!("用户删除成功：ID={}", id);
        Ok(true)
    } else {
        log_warn!("未找到ID为{}的用户。", id);
        Ok(false)
    }
}

// 主函数，执行一系列SQLite操作
#[allow(dead_code)]
pub fn run() -> Result<()> {
    // 数据库文件路径
    let db_path = "db/users.db";

    // 初始化数据库连接
    let conn = init_db(db_path)?;

    // 添加示例用户
    log_info!("\n=== 添加示例用户 ===");
    add_user(&conn, "张三", "zhangsan@example.com", 25)?;
    add_user(&conn, "李四", "lisi@example.com", 30)?;
    add_user(&conn, "王五", "wangwu@example.com", 35)?;

    // 查询所有用户
    log_info!("\n=== 查询所有用户 ===");
    let all_users = get_all_users(&conn)?;
    for user in &all_users {
        log_info!("{:?}", user);
    }

    // 根据ID查询用户
    log_info!("\n=== 根据ID查询用户 ===");
    if let Some(user) = get_user_by_id(&conn, 1)? {
        log_info!("查询结果: {:?}", user);
    }

    // 更新用户信息
    log_info!("\n=== 更新用户信息 ===");
    update_user(&conn, 1, "张三更新后", "zhangsan_new@example.com", 26)?;

    // 再次查询所有用户，查看更新结果
    log_info!("\n=== 更新后的所有用户 ===");
    let updated_users = get_all_users(&conn)?;
    for user in &updated_users {
        log_info!("{:?}", user);
    }

    // 删除用户
    log_info!("\n=== 删除用户 ===");
    delete_user(&conn, 2)?;

    // 最终查询所有用户，查看删除结果
    log_info!("\n=== 删除后的所有用户 ===");
    let final_users = get_all_users(&conn)?;
    for user in &final_users {
        log_info!("{:?}", user);
    }

    log_info!("\nSQLite操作完成！");

    Ok(())
}
