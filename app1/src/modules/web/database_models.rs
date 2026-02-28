use rusqlite::Result;

// Database 接口抽象：提供统一的数据库操作能力
pub trait DatabaseService {
    fn validate_user(&self, username: &str, password: &str) -> Result<bool>;
    fn get_user_info(&self, username: &str) -> Result<Option<(i32, String, String)>>;
    fn get_user_theme_config(&self, username: &str) -> Result<Option<String>>;
    fn update_user_theme_config(&self, username: &str, theme_config: &str) -> Result<()>;

    fn save_session(
        &self,
        session_id: &str,
        username: Option<&str>,
        room: Option<&str>,
    ) -> Result<()>;
    fn load_sessions(&self) -> Result<Vec<(String, Option<String>, Option<String>)>>;
    fn remove_session(&self, session_id: &str) -> Result<()>;

    fn save_message(
        &self,
        session_id: &str,
        username: &str,
        room: Option<&str>,
        content: &str,
    ) -> Result<()>;
    fn load_messages(
        &self,
        room: Option<&str>,
        limit: i32,
    ) -> Result<Vec<(String, String, Option<String>, String, String)>>;
}
