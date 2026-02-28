use bcrypt;
use chrono::Utc;
use rusqlite::{Connection, Error, Result, params};
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

// 数据库连接管理结构体
#[derive(Debug)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    // 创建新的数据库连接
    pub fn new(db_path: &str) -> Result<Self> {
        // 确保数据库文件存在的目录
        if let Some(parent) = Path::new(db_path).parent() {
            if !parent.exists() {
                if let Err(_err) = std::fs::create_dir_all(parent) {
                    // 创建目录失败，返回自定义数据库错误
                    return Err(Error::ExecuteReturnedResults);
                }
            }
        }

        let conn = Connection::open(db_path)?;

        // 初始化数据库表（如果不存在）
        conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE,
                password TEXT NOT NULL,
                email TEXT
            )",
            [],
        )?;

        // 创建websocket会话表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS ws_session (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL UNIQUE,
                username TEXT,
                room TEXT,
                connected_at TEXT NOT NULL,
                last_active TEXT NOT NULL
            )",
            [],
        )?;

        // 创建websocket消息表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS ws_message (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                username TEXT NOT NULL,
                room TEXT,
                content TEXT NOT NULL,
                timestamp TEXT NOT NULL
            )",
            [],
        )?;

        // 检查是否有用户，如果没有则创建一个默认用户用于测试
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
        if count == 0 {
            // 使用bcrypt算法加密存储默认用户密码
            let admin_pwd = "password123";
            let user_pwd = "user123";
            let admin_hash = bcrypt::hash(admin_pwd, bcrypt::DEFAULT_COST)
                .unwrap_or_else(|_| admin_pwd.to_string());
            let user_hash = bcrypt::hash(user_pwd, bcrypt::DEFAULT_COST)
                .unwrap_or_else(|_| user_pwd.to_string());
            conn.execute(
                "INSERT INTO users (username, password, email) VALUES (?1, ?2, ?3)",
                params!["admin", admin_hash, "admin@example.com"],
            )?;
            conn.execute(
                "INSERT INTO users (username, password, email) VALUES (?1, ?2, ?3)",
                params!["user1", user_hash, "user1@example.com"],
            )?;
        }

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    // 验证用户凭据（支持明文和bcrypt哈希）
    pub fn validate_user(&self, username: &str, password: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare("SELECT password FROM users WHERE username = ?1")?;
        let mut rows = stmt.query(params![username])?;

        if let Some(row) = rows.next()? {
            let stored: String = row.get(0)?;

            // 如果是bcrypt哈希（以$2开头），使用bcrypt验证；否则回退为明文比较
            if stored.starts_with("$2") {
                let valid = bcrypt::verify(password, &stored).unwrap_or(false);
                Ok(valid)
            } else {
                Ok(stored == password)
            }
        } else {
            Ok(false)
        }
    }

    // 获取用户信息（成功登录后）
    pub fn get_user_info(&self, username: &str) -> Result<Option<(i32, String, String)>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare("SELECT id, username, email FROM users WHERE username = ?")?;
        let mut rows = stmt.query([username])?;

        if let Some(row) = rows.next()? {
            let id: i32 = row.get(0)?;
            let username: String = row.get(1)?;
            let email: String = row.get(2)?;
            Ok(Some((id, username, email)))
        } else {
            Ok(None)
        }
    }

    // 保存会话信息
    pub fn save_session(
        &self,
        session_id: &str,
        username: Option<&str>,
        room: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT OR REPLACE INTO ws_session (session_id, username, room, connected_at, last_active) \
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![session_id, username, room, now],
        )?;

        Ok(())
    }

    // 加载所有会话信息
    pub fn load_sessions(&self) -> Result<Vec<(String, Option<String>, Option<String>)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT session_id, username, room FROM ws_session")?;

        let session_iter = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;

        let mut sessions = Vec::new();
        for session in session_iter {
            sessions.push(session?);
        }

        Ok(sessions)
    }

    // 移除会话
    pub fn remove_session(&self, session_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "DELETE FROM ws_session WHERE session_id = ?1",
            params![session_id],
        )?;

        Ok(())
    }

    // 保存消息
    pub fn save_message(
        &self,
        session_id: &str,
        username: &str,
        room: Option<&str>,
        content: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO ws_message (session_id, username, room, content, timestamp) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session_id, username, room, content, now],
        )?;

        Ok(())
    }

    // 加载消息列表
    pub fn load_messages(
        &self,
        room: Option<&str>,
        limit: i32,
    ) -> Result<Vec<(String, String, Option<String>, String, String)>> {
        let conn = self.conn.lock().unwrap();

        let query = match room {
            Some(_r) => {
                "SELECT session_id, username, room, content, timestamp FROM ws_message \
                         WHERE room = ?1 ORDER BY timestamp DESC LIMIT ?2"
            }
            None => {
                "SELECT session_id, username, room, content, timestamp FROM ws_message \
                     ORDER BY timestamp DESC LIMIT ?1"
            }
        };

        let mut stmt = conn.prepare(query)?;

        let message_iter: Box<
            dyn Iterator<
                Item = Result<(String, String, Option<String>, String, String), rusqlite::Error>,
            >,
        > = match room {
            Some(r) => Box::new(stmt.query_map(params![r, limit], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?),
            None => Box::new(stmt.query_map(params![limit], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?),
        };

        let mut messages = Vec::new();
        for message in message_iter {
            messages.push(message?);
        }

        // 反转消息顺序，让最早的消息在前面
        messages.reverse();

        Ok(messages)
    }

    // 获取用户主题配置
    pub fn get_user_theme_config(&self, username: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT theme_config FROM users WHERE username = ?1")?;
        let mut rows = stmt.query(params![username])?;

        if let Some(row) = rows.next()? {
            let theme_config: Option<String> = row.get(0)?;
            Ok(theme_config)
        } else {
            Ok(None)
        }
    }

    // 更新用户主题配置
    pub fn update_user_theme_config(&self, username: &str, theme_config: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE users SET theme_config = ?1 WHERE username = ?2",
            params![theme_config, username],
        )?;
        Ok(())
    }
}

// 实现接口抽象
impl super::database_models::DatabaseService for Database {
    fn validate_user(&self, username: &str, password: &str) -> Result<bool> {
        self.validate_user(username, password)
    }
    fn get_user_info(&self, username: &str) -> Result<Option<(i32, String, String)>> {
        self.get_user_info(username)
    }
    fn save_session(
        &self,
        session_id: &str,
        username: Option<&str>,
        room: Option<&str>,
    ) -> Result<()> {
        self.save_session(session_id, username, room)
    }
    fn load_sessions(&self) -> Result<Vec<(String, Option<String>, Option<String>)>> {
        self.load_sessions()
    }
    fn remove_session(&self, session_id: &str) -> Result<()> {
        self.remove_session(session_id)
    }
    fn save_message(
        &self,
        session_id: &str,
        username: &str,
        room: Option<&str>,
        content: &str,
    ) -> Result<()> {
        self.save_message(session_id, username, room, content)
    }
    fn load_messages(
        &self,
        room: Option<&str>,
        limit: i32,
    ) -> Result<Vec<(String, String, Option<String>, String, String)>> {
        self.load_messages(room, limit)
    }
    fn get_user_theme_config(&self, username: &str) -> Result<Option<String>> {
        self.get_user_theme_config(username)
    }
    fn update_user_theme_config(&self, username: &str, theme_config: &str) -> Result<()> {
        self.update_user_theme_config(username, theme_config)
    }
}
