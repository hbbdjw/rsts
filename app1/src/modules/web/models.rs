use crate::modules::ssh::russh_client::RusshClient;
use crate::modules::ssh::ssh2_pty_client::Ssh2PtyClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

// API 测试模块：端点列表简要信息
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ApiEndpointBrief {
    /// 端点唯一ID
    pub id: i64,
    /// 父节点ID（可为空）
    pub parent_id: Option<i64>,
    /// 端点名称
    pub name: String,
    /// HTTP 方法（GET/POST/PUT/DELETE等）
    pub method: String,
    /// 请求URL路径
    pub url: String,
    /// 排序序号（越小越靠前）
    pub order_index: i32,
}

// API 测试模块：端点详细信息
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ApiEndpointDetail {
    /// 端点唯一ID
    pub id: i64,
    /// 父节点ID（可为空）
    pub parent_id: Option<i64>,
    /// 端点名称
    pub name: String,
    /// HTTP 方法（GET/POST/PUT/DELETE等）
    pub method: String,
    /// 请求URL路径
    pub url: String,
    /// 请求头JSON（可为空）
    pub headers: Option<String>,
    /// 查询参数JSON（可为空）
    pub params: Option<String>,
    /// 请求体类型：raw/form/json 等（可为空）
    pub body_type: Option<String>,
    /// 请求体内容（可为空）
    pub body: Option<String>,
    /// Content-Type（可为空）
    pub content_type: Option<String>,
    /// 排序序号
    pub order_index: i32,
    /// 创建时间（ISO 8601 字符串）
    pub created_at: String,
    /// 更新时间（ISO 8601 字符串）
    pub updated_at: String,
}

// API 测试模块：创建端点请求体
#[derive(Deserialize, Debug)]
pub struct CreateEndpointPayload {
    /// 父节点ID（可为空）
    pub parent_id: Option<i64>,
    /// 端点名称
    pub name: String,
    /// HTTP 方法
    pub method: String,
    /// 请求URL路径
    pub url: String,
    /// 请求头JSON（可为空）
    pub headers: Option<String>,
    /// 查询参数JSON（可为空）
    pub params: Option<String>,
    /// 请求体类型（可为空）
    pub body_type: Option<String>,
    /// 请求体内容（可为空）
    pub body: Option<String>,
    /// Content-Type（可为空）
    pub content_type: Option<String>,
    /// 排序序号（可为空）
    pub order_index: Option<i32>,
}

// API 测试模块：更新端点请求体
#[derive(Deserialize, Debug)]
pub struct UpdateEndpointPayload {
    /// 父节点ID（可为空）
    pub parent_id: Option<i64>,
    /// 端点名称
    pub name: String,
    /// HTTP 方法
    pub method: String,
    /// 请求URL路径
    pub url: String,
    /// 请求头JSON（可为空）
    pub headers: Option<String>,
    /// 查询参数JSON（可为空）
    pub params: Option<String>,
    /// 请求体类型（可为空）
    pub body_type: Option<String>,
    /// 请求体内容（可为空）
    pub body: Option<String>,
    /// Content-Type（可为空）
    pub content_type: Option<String>,
    /// 排序序号（可为空）
    pub order_index: Option<i32>,
}

// SQLite 模块：数据库文件信息
#[derive(Serialize, Debug)]
pub struct DatabaseInfo {
    /// 数据库文件名（含扩展名）
    pub name: String,
    /// 文件系统路径
    pub path: String,
}

// SQLite 模块：表信息
#[derive(Serialize, Debug)]
pub struct TableInfo {
    /// 表名
    pub name: String,
    /// 列信息列表
    pub columns: Vec<ColumnInfo>,
}

// SQLite 模块：列信息
#[derive(Serialize, Debug)]
pub struct ColumnInfo {
    /// 列名
    pub name: String,
    /// 数据类型（如 TEXT/INTEGER/REAL）
    pub data_type: String,
    /// 是否非空
    pub not_null: bool,
    /// 是否主键
    pub primary_key: bool,
}

// SQLite 模块：分页查询参数
#[derive(Deserialize, Debug)]
pub struct QueryParams {
    /// 数据库名
    pub db_name: String,
    /// 表名（可选，省略则查询所有表）
    pub table_name: Option<String>,
    /// 页码（从1开始，可选）
    pub page: Option<u32>,
    /// 每页条数（可选）
    pub page_size: Option<u32>,
}

// SQLite 模块：分页查询结果
#[derive(Serialize, Debug)]
pub struct PaginationResult {
    /// 数据记录列表（JSON）
    pub data: Vec<serde_json::Value>,
    /// 总记录数
    pub total: u64,
    /// 当前页码
    pub page: u32,
    /// 每页条数
    pub page_size: u32,
    /// 总页数
    pub total_pages: u32,
}

// SQLite 模块：创建表列定义
#[derive(Deserialize, Debug)]
pub struct CreateTableColumn {
    /// 列名
    pub name: String,
    /// 数据类型
    pub data_type: String,
    /// 是否非空（可选）
    pub not_null: Option<bool>,
    /// 是否主键（可选）
    pub primary_key: Option<bool>,
}

// SQLite 模块：创建表请求体
#[derive(Deserialize, Debug)]
pub struct CreateTablePayload {
    /// 数据库名
    pub db_name: String,
    /// 表名
    pub table_name: String,
    /// 列定义列表
    pub columns: Vec<CreateTableColumn>,
}

// SQLite 模块：删除表请求体
#[derive(Deserialize, Debug)]
pub struct DropTablePayload {
    /// 数据库名
    pub db_name: String,
    /// 表名
    pub table_name: String,
}

// SQLite 模块：重命名表请求体
#[derive(Deserialize, Debug)]
pub struct RenameTablePayload {
    /// 数据库名
    pub db_name: String,
    /// 旧表名
    pub table_name: String,
    /// 新表名
    pub new_name: String,
}

// SQLite 模块：重命名列请求体
#[derive(Deserialize, Debug)]
pub struct RenameColumnPayload {
    /// 数据库名
    pub db_name: String,
    /// 表名
    pub table_name: String,
    /// 旧列名
    pub old_name: String,
    /// 新列名
    pub new_name: String,
}

// SQLite 模块：新增列请求体
#[derive(Deserialize, Debug)]
pub struct AddColumnPayload {
    /// 数据库名
    pub db_name: String,
    /// 表名
    pub table_name: String,
    /// 列名
    pub name: String,
    /// 数据类型
    pub data_type: String,
    /// 是否非空（可选）
    pub not_null: Option<bool>,
    /// 默认值（JSON，可选）
    pub default: Option<serde_json::Value>,
}

// SQLite 模块：删除列请求体
#[derive(Deserialize, Debug)]
pub struct DropColumnPayload {
    /// 数据库名
    pub db_name: String,
    /// 表名
    pub table_name: String,
    /// 列名
    pub column_name: String,
}

// SQLite 模块：插入行请求体
#[derive(Deserialize, Debug)]
pub struct RowInsertPayload {
    /// 数据库名
    pub db_name: String,
    /// 表名
    pub table_name: String,
    /// 插入的键值对
    pub values: serde_json::Map<String, serde_json::Value>,
}

// SQLite 模块：更新行请求体
#[derive(Deserialize, Debug)]
pub struct RowUpdatePayload {
    /// 数据库名
    pub db_name: String,
    /// 表名
    pub table_name: String,
    /// 主键列名
    pub pk_column: String,
    /// 主键值
    pub pk_value: serde_json::Value,
    /// 更新的键值对
    pub values: serde_json::Map<String, serde_json::Value>,
}

// SQLite 模块：删除单行请求体
#[derive(Deserialize, Debug)]
pub struct RowDeletePayload {
    /// 数据库名
    pub db_name: String,
    /// 表名
    pub table_name: String,
    /// 主键列名
    pub pk_column: String,
    /// 主键值
    pub pk_value: serde_json::Value,
}

// SQLite 模块：批量删除行请求体
#[derive(Deserialize, Debug)]
pub struct RowBatchDeletePayload {
    /// 数据库名
    pub db_name: String,
    /// 表名
    pub table_name: String,
    /// 主键列名
    pub pk_column: String,
    /// 待删除的主键值列表
    pub pk_values: Vec<serde_json::Value>,
}

// SQLite 模块：SQL 控制台请求体
#[derive(Deserialize, Debug)]
pub struct SqlQueryPayload {
    /// 数据库名
    pub db_name: String,
    /// SQL 语句
    pub sql: String,
    /// 可选参数列表
    pub params: Option<Vec<serde_json::Value>>,
}

// SFTP 模块：查询路径请求参数
#[derive(Deserialize, Debug)]
pub struct PathQuery {
    /// 会话ID
    pub session_id: usize,
    /// 路径（为空时取根路径）
    pub path: Option<String>,
}

// SFTP 模块：创建会话请求体
#[derive(Deserialize, Debug)]
pub struct CreateSessionPayload {
    /// 主机名
    pub hostname: String,
    /// 端口
    pub port: u16,
    /// 用户名
    pub username: String,
    /// 密码
    pub password: String,
}

// SFTP 模块：删除文件请求体
#[derive(Deserialize, Debug)]
pub struct DeletePayload {
    /// 会话ID
    pub session_id: usize,
    /// 文件/目录路径
    pub path: String,
}

// SFTP 模块：重命名请求体
#[derive(Deserialize, Debug)]
pub struct RenamePayload {
    /// 会话ID
    pub session_id: usize,
    /// 原路径
    pub path: String,
    /// 新名称
    pub new_name: String,
}

// SFTP 模块：写文件请求体
#[derive(Deserialize, Debug)]
pub struct WriteFilePayload {
    /// 会话ID
    pub session_id: usize,
    /// 文件路径
    pub path: String,
    /// 文件内容
    pub content: String,
}

// SFTP 模块：上传文件请求体
#[derive(Deserialize, Debug)]
pub struct UploadPayload {
    /// 会话ID
    pub session_id: usize,
    /// 目录路径
    pub path: String,
    /// 文件名
    pub filename: String,
    /// 文件内容（base64）
    pub content_base64: String,
}

// SFTP 模块：创建目录请求体
#[derive(Deserialize, Debug)]
pub struct MkdirPayload {
    /// 会话ID
    pub session_id: usize,
    /// 目录路径
    pub path: String,
}

// SFTP 模块：修改权限请求体
#[derive(Deserialize, Debug)]
pub struct ChmodPayload {
    /// 会话ID
    pub session_id: usize,
    /// 文件/目录路径
    pub path: String,
    /// 权限模式（例如 0o755 = 493）
    pub mode: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SshGroup {
    pub id: i64,
    pub name: String,
    pub is_default: i32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SshGroupInput {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SshServer {
    pub id: i64,
    pub alias: String,
    pub hostname: String,
    pub port: i32,
    pub username: String,
    pub password: Option<String>,
    pub group_id: i64,
    pub remark: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SshServerInput {
    pub alias: Option<String>,
    pub hostname: String,
    pub port: Option<i32>,
    pub username: String,
    pub password: Option<String>,
    pub group_id: Option<i64>,
    pub remark: Option<String>,
}

// 聊天上传媒体：请求体
#[derive(Deserialize, Debug)]
pub struct UploadChatMediaPayload {
    /// 文件名
    pub filename: String,
    /// base64 内容（可含 data URL 前缀）
    pub content_base64: String,
    /// 媒体类型：image | video | audio
    pub media_type: String,
}

// 登录：请求体
#[derive(Debug, Deserialize, Serialize)]
pub struct LoginRequest {
    /// 用户名
    pub username: String,
    /// 密码
    pub password: String,
}

// 登录：响应体
#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    /// 是否成功
    pub code: String,
    /// 提示信息
    pub msg: String,
    /// 返回数据
    pub data: Option<serde_json::Value>,
}

// 登录：用户信息
#[derive(Debug, Serialize)]
pub struct UserInfo {
    /// 用户ID
    pub id: i32,
    /// 用户名
    pub username: String,
    /// 邮箱
    pub email: String,
}

// WebSocket：服务器发送给会话的消息
#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct Message(pub String);

// WebSocket：创建新会话
#[derive(actix::Message)]
#[rtype(u64)]
pub struct Connect {
    /// 会话地址（用于发送消息）
    pub addr: actix::Recipient<Message>,
}

// WebSocket：会话断开
#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    /// 会话ID
    pub id: u64,
}

// WebSocket：客户端消息（房间内广播）
#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct ClientMessage {
    /// 客户端会话ID
    pub id: u64,
    /// 消息内容
    pub msg: String,
    /// 房间名称
    pub room: String,
}

// WebSocket：设置用户名
#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct SetUsername {
    /// 会话ID
    pub id: u64,
    /// 用户名
    pub username: String,
}

// WebSocket：私聊消息
#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct PrivateMessage {
    /// 发送者ID
    pub sender_id: u64,
    /// 接收者用户名
    pub receiver_username: String,
    /// 消息内容
    pub msg: String,
}

// WebSocket：请求历史消息
#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct GetHistoryMessages {
    /// 房间名称
    pub room: String,
    /// 请求数量限制
    pub limit: i32,
    /// 请求者会话ID
    pub requester_id: u64,
}

// WebSocket：列出房间
pub struct ListRooms;

impl actix::Message for ListRooms {
    type Result = Vec<String>;
}

// WebSocket：加入房间
#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct Join {
    /// 客户端ID
    pub id: u64,
    /// 房间名称
    pub name: String,
}

// SSH WebSocket：连接请求
#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct ConnectSsh {
    /// 连接凭据
    pub credentials: crate::modules::ssh::SshCredentials,
}

// SSH WebSocket：设置客户端ID
#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct SetSshClientId {
    /// 客户端ID
    pub client_id: usize,
}

// SSH WebSocket：执行命令
#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct ExecuteCommand {
    /// 命令字符串
    pub command: String,
}

// WebSocket：通用文本消息（SSH/PTY模块复用）
#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct SendWsMessage {
    pub text: String,
}

// SSH PTY：连接凭据（用于PTY客户端）
#[derive(Deserialize, Debug, Clone)]
pub struct SshCredentials {
    /// 主机名
    pub hostname: String,
    /// 端口
    pub port: u16,
    /// 用户名
    pub username: String,
    /// 密码
    pub password: String,
}

// SSH PTY：设置客户端（会话内部状态更新）
#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct SetSshClient {
    /// 任意PTY客户端封装
    pub client: AnyPtyClient,
    /// 是否已连接
    pub connected: bool,
}

// SSH PTY：客户端封装（支持 russh 与 ssh2 实现）
#[derive(Clone)]
pub enum AnyPtyClient {
    /// russh 客户端
    #[allow(dead_code)]
    Russh(Arc<Mutex<RusshClient>>),
    /// ssh2 PTY 客户端
    Ssh2(Arc<Mutex<Ssh2PtyClient>>),
}
