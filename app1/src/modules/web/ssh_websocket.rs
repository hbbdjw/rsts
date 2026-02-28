use crate::{log_error, log_info};
use actix::{ActorContext, AsyncContext, Handler};
use actix_web_actors::ws;
use serde::Deserialize;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

// 使用真实的SSH模块
use super::actors::WsSshSession;
use super::models::{ConnectSsh, ExecuteCommand, SendWsMessage, SetSshClientId};
use crate::modules::ssh::{SshCredentials, SshService};

// Actor 类型已集中到 actors.rs

impl WsSshSession {
    pub fn new(ssh_service: Arc<Mutex<SshService>>) -> Self {
        Self {
            hb: Instant::now(),
            ssh_service,
            ssh_client_id: None,
            connected_to_ssh: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 发送心跳包
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // 检查客户端心跳
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                log_info!("SSH WebSocket客户端心跳超时，断开连接");

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
        if let Some(client_id) = self.ssh_client_id.take() {
            // 在新的任务中断开SSH连接，避免阻塞WebSocket处理
            let ssh_service = Arc::clone(&self.ssh_service);
            let client_id_clone = client_id.clone();
            let connected_flag = Arc::clone(&self.connected_to_ssh);

            actix::spawn(async move {
                let ssh_service = ssh_service.lock().await;
                if let Err(e) = ssh_service.close_connection(client_id_clone).await {
                    log_error!("关闭SSH连接失败: {}", e);
                }
                connected_flag.store(false, std::sync::atomic::Ordering::Relaxed);
            });

            self.connected_to_ssh
                .store(false, std::sync::atomic::Ordering::Relaxed);

            // 发送断开连接消息
            let disconnect_msg =
                serde_json::json!({"type": "disconnected", "message": "SSH连接已断开"});
            ctx.text(disconnect_msg.to_string());

            log_info!("SSH连接已断开: {}", client_id);
        }
    }
    /// 发送错误消息到客户端
    fn send_error(&self, message: &str, ctx: &mut ws::WebsocketContext<Self>) {
        let error_msg = serde_json::json!({"type": "error", "message": message});
        ctx.text(error_msg.to_string());
    }
}

/// 消息类型枚举 - 从客户端接收的消息类型
#[derive(Deserialize, Debug)]
pub enum ClientMessageType {
    #[serde(rename = "connect")]
    Connect,
    #[serde(rename = "command")]
    Command,
    #[serde(rename = "disconnect")]
    Disconnect,
    #[serde(other)]
    Unknown,
}

impl actix::Actor for WsSshSession {
    type Context = ws::WebsocketContext<Self>;

    /// 在Actor启动时调用
    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
        log_info!("SSH WebSocket会话已启动");
    }

    /// 在Actor停止时调用
    fn stopping(&mut self, ctx: &mut Self::Context) -> actix::Running {
        log_info!("SSH WebSocket会话正在停止");

        // 断开SSH连接
        self.disconnect_ssh(ctx);

        actix::Running::Stop
    }
}

/// 处理WebSocket流消息
impl actix::StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSshSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(msg) => self.handle_message(msg, ctx),
            Err(err) => {
                log_error!("WebSocket协议错误: {:?}", err);
                ctx.stop();
            }
        }
    }
}

/// 处理WebSocket具体消息
impl WsSshSession {
    fn handle_message(&mut self, msg: ws::Message, ctx: &mut ws::WebsocketContext<Self>) {
        match msg {
            ws::Message::Ping(msg) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.hb = Instant::now();
            }
            ws::Message::Text(text) => {
                log_info!("收到SSH WebSocket文本消息: {}", text);

                // 尝试解析JSON消息
                if let Ok(json_msg) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(msg_type) = json_msg.get("type").and_then(|t| t.as_str()) {
                        match msg_type {
                            "connect" => {
                                // 处理连接请求
                                let host =
                                    json_msg.get("host").and_then(|h| h.as_str()).unwrap_or("");
                                let port =
                                    json_msg.get("port").and_then(|p| p.as_u64()).unwrap_or(22)
                                        as u16;
                                let username = json_msg
                                    .get("username")
                                    .and_then(|u| u.as_str())
                                    .unwrap_or("");
                                let password = json_msg
                                    .get("password")
                                    .and_then(|p| p.as_str())
                                    .unwrap_or("");

                                if !host.is_empty() && !username.is_empty() {
                                    // 创建凭证
                                    let credentials = SshCredentials {
                                        hostname: host.to_string(),
                                        port,
                                        username: username.to_string(),
                                        password: password.to_string(),
                                    };

                                    // 在异步上下文中处理连接
                                    let addr = ctx.address();
                                    let credentials_clone = credentials.clone();

                                    actix::spawn(async move {
                                        let _ = addr
                                            .send(ConnectSsh {
                                                credentials: credentials_clone,
                                            })
                                            .await;
                                    });
                                } else {
                                    self.send_error("主机地址和用户名不能为空", ctx);
                                }
                            }
                            "command" => {
                                // 处理命令请求
                                if let Some(command) =
                                    json_msg.get("command").and_then(|c| c.as_str())
                                {
                                    // 在异步上下文中处理命令执行
                                    let command_clone = command.to_string();
                                    let addr = ctx.address();

                                    actix::spawn(async move {
                                        let _session = addr
                                            .send(ExecuteCommand {
                                                command: command_clone,
                                            })
                                            .await;
                                    });
                                } else {
                                    self.send_error("命令不能为空", ctx);
                                }
                            }
                            "disconnect" => {
                                // 处理断开连接请求
                                self.disconnect_ssh(ctx);
                            }
                            _ => {
                                self.send_error(&format!("未知的消息类型: {}", msg_type), ctx);
                            }
                        }
                    } else {
                        self.send_error("消息缺少type字段", ctx);
                    }
                } else {
                    self.send_error("无法解析JSON消息", ctx);
                }
            }
            ws::Message::Binary(_bin) => {
                log_info!("收到SSH WebSocket二进制消息");
                ctx.text("二进制消息暂不支持");
            }
            ws::Message::Close(reason) => {
                log_info!("SSH WebSocket连接关闭: {:?}", reason);
                ctx.close(reason);
                ctx.stop();
            }
            ws::Message::Continuation(_) => {
                log_info!("收到SSH WebSocket continuation消息");
                ctx.stop();
            }
            ws::Message::Nop => {
                log_info!("收到SSH WebSocket NOP消息");
            }
        }
    }
}

/// 处理连接SSH消息
impl Handler<ConnectSsh> for WsSshSession {
    type Result = ();

    fn handle(&mut self, msg: ConnectSsh, ctx: &mut Self::Context) {
        let credentials = msg.credentials;

        // 创建一个新的任务来处理异步连接
        let ssh_service = Arc::clone(&self.ssh_service);
        let addr = ctx.address();
        let credentials_clone = credentials.clone();

        actix::spawn(async move {
            let ssh_service = ssh_service.lock().await;

            match ssh_service
                .create_connection(credentials_clone.clone())
                .await
            {
                Ok(client_id) => {
                    // 更新会话状态
                    addr.do_send(SetSshClientId { client_id });

                    // 发送连接成功消息
                    let msg = serde_json::json!({"type": "connected", "prompt": "$ ", "host": credentials_clone.hostname, "port": credentials_clone.port, "username": credentials_clone.username});
                    addr.do_send(SendWsMessage {
                        text: msg.to_string(),
                    });
                }
                Err(err) => {
                    // 发送连接失败消息
                    let msg = serde_json::json!({"type": "error", "message": format!("SSH连接失败: {:?}", err)});
                    addr.do_send(SendWsMessage {
                        text: msg.to_string(),
                    });
                }
            }
        });
    }
}

// 数据模型已迁移至 models.rs

// 数据模型已迁移至 models.rs

// 数据模型已迁移至 models.rs

/// 处理设置SSH客户端ID消息
impl Handler<SetSshClientId> for WsSshSession {
    type Result = ();

    fn handle(&mut self, msg: SetSshClientId, _ctx: &mut Self::Context) {
        self.ssh_client_id = Some(msg.client_id);
        self.connected_to_ssh
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// 处理执行命令消息
impl Handler<ExecuteCommand> for WsSshSession {
    type Result = ();

    fn handle(&mut self, msg: ExecuteCommand, ctx: &mut Self::Context) {
        if let Some(client_id) = &self.ssh_client_id {
            let command = msg.command;
            let ssh_service = Arc::clone(&self.ssh_service);
            let client_id_clone = client_id.clone();
            let addr = ctx.address();

            actix::spawn(async move {
                let ssh_service = ssh_service.lock().await;

                if let Ok(client) = ssh_service.get_client(client_id_clone).await {
                    match client.lock().await.execute_command(&command).await {
                        Ok(output) => {
                            // 发送命令输出
                            let msg = serde_json::json!({"type": "output", "content": output});
                            let text = msg.to_string();
                            addr.do_send(SendWsMessage { text });

                            // 发送提示符
                            let msg = serde_json::json!({"type": "prompt", "prompt": "$ "});
                            let text = msg.to_string();
                            addr.do_send(SendWsMessage { text });
                        }
                        Err(err) => {
                            // 发送错误消息
                            let msg = serde_json::json!({"type": "error", "message": format!("命令执行失败: {:?}", err)});
                            let text = msg.to_string();
                            addr.do_send(SendWsMessage { text });
                        }
                    }
                } else {
                    // 发送错误消息
                    let msg = serde_json::json!({"type": "error", "message": "SSH客户端不存在或已断开连接"});
                    let text = msg.to_string();
                    addr.do_send(SendWsMessage { text });
                }
            });
        } else {
            // 发送错误消息
            let msg = serde_json::json!({"type": "error", "message": "未连接到SSH服务器"});
            let text = msg.to_string();
            ctx.text(text);
        }
    }
}

/// 处理发送WebSocket消息
impl Handler<SendWsMessage> for WsSshSession {
    type Result = ();

    fn handle(&mut self, msg: SendWsMessage, ctx: &mut Self::Context) {
        ctx.text(msg.text);
    }
}

// 心跳间隔
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

// 客户端超时时间
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// SSH WebSocket入口点
pub async fn ssh_route(
    req: actix_web::HttpRequest,
    stream: actix_web::web::Payload,
) -> Result<actix_web::HttpResponse, actix_web::Error> {
    log_info!("=== SSH路由函数被调用 ===");
    log_info!(
        "SSH WebSocket连接请求 - 路径: {}, 方法: {}",
        req.path(),
        req.method()
    );
    log_info!("请求头: {:?}", req.headers());

    // 检查是否为WebSocket升级请求
    let connection_header = req.headers().get("connection");
    let upgrade_header = req.headers().get("upgrade");
    log_info!("Connection头: {:?}", connection_header);
    log_info!("Upgrade头: {:?}", upgrade_header);

    // 创建SSH服务实例
    let ssh_service = Arc::new(Mutex::new(SshService::new()));

    match ws::start(WsSshSession::new(ssh_service), &req, stream) {
        Ok(response) => {
            log_info!("SSH WebSocket连接成功建立");
            Ok(response)
        }
        Err(err) => {
            log_error!("SSH WebSocket连接失败: {:?}", err);
            Err(err)
        }
    }
}
