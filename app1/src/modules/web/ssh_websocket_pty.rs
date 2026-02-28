use super::actors::WsSshPtySession;
use super::models::{AnyPtyClient, SendWsMessage, SetSshClient, SshCredentials};
use crate::modules::ssh::ssh2_pty_client::Ssh2PtyClient;
use actix::{Actor, ActorContext, AsyncContext, StreamHandler};
use actix_web_actors::ws;
use log::{error, info};
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, mpsc};

// Actor 类型已集中到 actors.rs

impl WsSshPtySession {
    pub fn new() -> Self {
        Self {
            hb: Instant::now(),
            ssh_client: None,
            connected: false,
        }
    }

    /// 发送心跳包
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // 检查客户端心跳
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                info!("SSH WebSocket客户端心跳超时，断开连接");

                // 断开SSH连接
                act.disconnect_ssh(ctx);

                // 停止actor
                ctx.stop();
                return;
            }

            ctx.ping(b"");
        });
    }

    /// 断开SSH连接
    fn disconnect_ssh(&mut self, ctx: &mut ws::WebsocketContext<Self>) {
        if let Some(client) = self.ssh_client.take() {
            actix::spawn(async move {
                if let Err(e) = client.close().await {
                    error!("关闭SSH连接失败: {}", e);
                }
            });
        }

        self.connected = false;

        // 发送断开连接消息
        let disconnected_msg = serde_json::json!({"type": "disconnected"});
        ctx.text(disconnected_msg.to_string());

        info!("SSH连接已断开");
    }

    /// 发送错误消息
    fn send_error(&self, message: &str, ctx: &mut ws::WebsocketContext<Self>) {
        let error_msg = serde_json::json!({"type": "error", "message": message});
        ctx.text(error_msg.to_string());
    }

    /// 处理连接请求
    fn handle_connect(
        &mut self,
        credentials: SshCredentials,
        pty_cols: Option<u32>,
        pty_rows: Option<u32>,
        ctx: &mut ws::WebsocketContext<Self>,
    ) {
        if self.connected {
            self.send_error("已有活跃的SSH连接，请先断开", ctx);
            return;
        }

        info!(
            "尝试连接到SSH服务器: {}@{}:{}",
            credentials.username, credentials.hostname, credentials.port
        );

        let addr = ctx.address();
        let creds = credentials.clone();

        // 在新的任务中处理SSH连接
        tokio::spawn(async move {
            // 直接使用Ssh2PtyClient，因为RusshClient连接成功率较低
            let mut client = Ssh2PtyClient::new(
                creds.hostname.clone(),
                creds.port,
                creds.username.clone(),
                creds.password.clone(),
            );
            if let Err(e) = client.connect().await {
                let error_msg =
                    serde_json::json!({"type": "error", "message": format!("SSH连接失败: {}", e)});
                addr.do_send(SendWsMessage {
                    text: error_msg.to_string(),
                });
                return;
            }
            if let Err(e) = client.create_pty_session(pty_cols, pty_rows).await {
                let error_msg = serde_json::json!({"type": "error", "message": format!("创建PTY会话失败: {}", e)});
                addr.do_send(SendWsMessage {
                    text: error_msg.to_string(),
                });
                return;
            }
            let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
            if let Err(e) = client.start_pty_io(tx).await {
                let error_msg = serde_json::json!({"type": "error", "message": format!("启动PTY IO失败: {}", e)});
                addr.do_send(SendWsMessage {
                    text: error_msg.to_string(),
                });
                return;
            }
            let client_arc = Arc::new(Mutex::new(client));
            addr.do_send(SetSshClient {
                client: AnyPtyClient::Ssh2(client_arc.clone()),
                connected: true,
            });
            let connected_msg = serde_json::json!({"type": "connected","host": creds.hostname,"port": creds.port,"username": creds.username});
            addr.do_send(SendWsMessage {
                text: connected_msg.to_string(),
            });
            while let Some(data) = rx.recv().await {
                let s = String::from_utf8_lossy(&data);
                let output_msg = serde_json::json!({"type": "output","data": s, "content": s});
                addr.do_send(SendWsMessage {
                    text: output_msg.to_string(),
                });
            }
        });
    }

    /// 处理输入
    fn handle_input(&self, data: String, ctx: &mut ws::WebsocketContext<Self>) {
        info!(
            "处理输入数据，连接状态: {}, 客户端状态: {:?}",
            self.connected,
            self.ssh_client.is_some()
        );

        if !self.connected {
            error!("SSH连接未建立，无法发送输入");
            self.send_error("SSH连接未建立", ctx);
            return;
        }

        if let Some(ref client) = self.ssh_client {
            let to_send = data.clone();
            info!(
                "向SSH客户端发送输入数据: {:?}, 数据长度: {}字节",
                to_send,
                to_send.len()
            );
            let client = client.clone();
            tokio::spawn(async move {
                info!("开始向PTY写入数据");
                if let Err(e) = client.write_to_pty(to_send.as_bytes()).await {
                    error!("向PTY发送输入失败: {}", e);
                } else {
                    info!("向PTY发送输入成功: {:?}", to_send);
                }
            });
        } else {
            error!("SSH客户端未初始化");
            self.send_error("SSH客户端未初始化", ctx);
        }
    }

    /// 处理窗口大小调整
    fn handle_resize(&self, width: u32, height: u32, ctx: &mut ws::WebsocketContext<Self>) {
        if !self.connected {
            self.send_error("SSH连接未建立", ctx);
            return;
        }

        if let Some(ref client) = self.ssh_client {
            let client = client.clone();
            tokio::spawn(async move {
                if let Err(e) = client.resize_pty(width, height).await {
                    error!("调整PTY窗口大小失败: {}", e);
                }
            });
        }
    }
}

// 数据模型已迁移至 models.rs

/// 客户端消息类型
#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "connect")]
    Connect {
        credentials: SshCredentials,
        col_width: Option<u32>,
        row_height: Option<u32>,
    },
    #[serde(rename = "input")]
    Input {
        #[serde(alias = "content")]
        data: String,
    },
    #[serde(rename = "resize")]
    Resize { width: u32, height: u32 },
    #[serde(rename = "disconnect")]
    Disconnect,
}

impl Actor for WsSshPtySession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("SSH PTY WebSocket会话开始");
        self.hb(ctx);
    }

    fn stopping(&mut self, ctx: &mut Self::Context) -> actix::Running {
        info!("SSH PTY WebSocket会话结束");
        self.disconnect_ssh(ctx);
        actix::Running::Stop
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSshPtySession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                self.hb = Instant::now();
                self.handle_text_message(text.to_string(), ctx);
            }
            Ok(ws::Message::Binary(bytes)) => {
                self.hb = Instant::now();
                let text = String::from_utf8_lossy(&bytes).to_string();
                self.handle_text_message(text, ctx);
            }
            Ok(ws::Message::Close(reason)) => {
                info!("WebSocket连接关闭: {:?}", reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

impl WsSshPtySession {
    fn handle_text_message(&mut self, text: String, ctx: &mut ws::WebsocketContext<Self>) {
        // 注意：不能使用 trim() 判断空文本，否则会把仅包含 "\r"/"\n" 的输入当作空白而丢弃，导致命令无法执行
        if text.is_empty() {
            info!("收到空文本消息，忽略");
            return;
        }

        info!("收到WebSocket文本消息: {:?}", text);

        // 优先尝试解析为JSON协议（忽略前导空白以提升容错）
        let starts_with_json = text.trim_start().starts_with('{');
        info!("消息是否以JSON开始: {}", starts_with_json);

        if starts_with_json {
            match serde_json::from_str::<ClientMessage>(text.trim()) {
                Ok(msg) => {
                    info!("成功解析JSON消息: {:?}", msg);
                    match msg {
                        ClientMessage::Connect {
                            credentials,
                            col_width,
                            row_height,
                        } => {
                            self.handle_connect(credentials, col_width, row_height, ctx);
                        }
                        ClientMessage::Input { data } => {
                            self.handle_input(data, ctx);
                        }
                        ClientMessage::Resize { width, height } => {
                            self.handle_resize(width, height, ctx);
                        }
                        ClientMessage::Disconnect => {
                            self.disconnect_ssh(ctx);
                        }
                    }
                }
                Err(e) => {
                    // 解析失败则降级为原始输入
                    error!("解析客户端JSON消息失败: {}，降级为原始输入", e);
                    self.handle_input(text, ctx);
                }
            }
        } else {
            // 非JSON文本，按原始输入处理（用于健壮性）
            info!("非JSON文本，按原始输入处理");
            self.handle_input(text, ctx);
        }
    }
}

// 数据模型已迁移至 models.rs

// Actor消息处理器
impl actix::Handler<SetSshClient> for WsSshPtySession {
    type Result = ();

    fn handle(&mut self, msg: SetSshClient, _ctx: &mut Self::Context) {
        self.ssh_client = Some(msg.client);
        self.connected = msg.connected;
    }
}

impl actix::Handler<SendWsMessage> for WsSshPtySession {
    type Result = ();

    fn handle(&mut self, msg: SendWsMessage, ctx: &mut Self::Context) {
        ctx.text(msg.text);
    }
}

// 常量定义
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// SSH PTY WebSocket路由处理器
pub async fn ssh_pty_route(
    req: actix_web::HttpRequest,
    stream: actix_web::web::Payload,
) -> Result<actix_web::HttpResponse, actix_web::Error> {
    let resp = ws::start(WsSshPtySession::new(), &req, stream);
    info!("新的SSH PTY WebSocket连接: {:?}", req.peer_addr());
    resp
}

// 数据模型已迁移至 models.rs

impl AnyPtyClient {
    pub async fn write_to_pty(&self, data: &[u8]) -> anyhow::Result<()> {
        match self {
            AnyPtyClient::Russh(c) => c.lock().await.write_to_pty(data).await,
            AnyPtyClient::Ssh2(c) => c.lock().await.write_to_pty(data).await,
        }
    }
    pub async fn resize_pty(&self, width: u32, height: u32) -> anyhow::Result<()> {
        match self {
            AnyPtyClient::Russh(c) => c.lock().await.resize_pty(width, height).await,
            AnyPtyClient::Ssh2(c) => c.lock().await.resize_pty(width, height).await,
        }
    }
    pub async fn close(&self) -> anyhow::Result<()> {
        match self {
            AnyPtyClient::Russh(c) => c.lock().await.close().await,
            AnyPtyClient::Ssh2(c) => c.lock().await.close().await,
        }
    }
}
