use actix::prelude::*;
use actix_web_actors::ws;
use std::time::{Duration, Instant};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use super::actors::{ChatServer, WsChatSession};
use super::database::Database;
use super::models::{
    ClientMessage, Connect, Disconnect, GetHistoryMessages, Join, ListRooms, Message,
    PrivateMessage, SetUsername,
};
use crate::log_error;
use crate::{log_debug, log_info};
use uuid::Uuid;
// 数据模型已迁移至 models.rs

/// `ChatServer` 管理聊天室并负责协调聊天会话
///
// Actor 类型已集中到 actors.rs

impl ChatServer {
    pub fn new(visitor_count: Arc<AtomicUsize>, db: Arc<Database>) -> ChatServer {
        // 默认房间
        let mut rooms = HashMap::new();
        rooms.insert("main".to_owned(), HashSet::new());

        ChatServer {
            sessions: HashMap::new(),
            rooms,
            visitor_count,
            db,
            id_to_session_id: HashMap::new(),
            id_to_username: HashMap::new(),
        }
    }
}

impl ChatServer {
    /// 向房间中的所有用户发送消息
    fn send_message(&self, room: &str, message: &str, skip_id: u64) {
        if let Some(sessions) = self.rooms.get(room) {
            for id in sessions {
                if *id != skip_id {
                    if let Some(addr) = self.sessions.get(id) {
                        addr.do_send(Message(message.to_owned()));
                    }
                }
            }
        }

        // 保存消息到数据库
        if let Some(session_id) = self.id_to_session_id.get(&skip_id) {
            if let Some(username) = self.id_to_username.get(&skip_id) {
                if let Err(err) = self
                    .db
                    .save_message(session_id, username, Some(room), message)
                {
                    log_error!("Failed to save message: {}", err);
                }
            }
        }
    }
}

/// 为 `ChatServer` 创建actor
impl Actor for ChatServer {
    /// 我们将使用简单的Context，只需要与其他actor通信的能力
    type Context = Context<Self>;

    /// 在actor启动时调用此方法
    /// 加载之前保存的会话信息
    fn started(&mut self, _ctx: &mut Self::Context) {
        log_info!("正在加载之前的会话信息...");

        match self.db.load_sessions() {
            Ok(sessions) => {
                // 由于WebSockets是有状态的，我们无法直接恢复之前的连接
                // 但可以加载会话信息，以便了解历史连接情况
                log_info!("加载了 {} 个历史会话记录", sessions.len());

                // 在实际应用中，你可能需要根据业务需求处理这些会话信息
                // 例如，可以显示一个"会话历史"区域给新连接的用户
            }
            Err(err) => {
                log_error!("加载会话信息失败: {}", err);
            }
        }

        log_info!("ChatServer启动完成");
    }
}

/// Connect消息处理器
///
/// 注册新会话并为此会话分配唯一ID
impl Handler<Connect> for ChatServer {
    type Result = u64;

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        log_info!("有人加入");
        // 通知同一房间的所有用户
        self.send_message("main", "有人加入", 0);

        // 使用随机ID注册会话
        let id = rand::random::<u64>();
        self.sessions.insert(id, msg.addr);

        // 自动将会话加入主房间
        self.rooms.entry("main".to_owned()).or_default().insert(id);

        // 生成唯一会话ID
        // 需要先添加 uuid 依赖并引入模块，这里假设使用 uuid 库
        let session_id = Uuid::new_v4().to_string();
        self.id_to_session_id.insert(id, session_id.clone());

        // 保存会话信息到数据库
        if let Err(err) = self.db.save_session(&session_id, None, Some("main")) {
            log_error!("Failed to save session: {}", err);
        }

        let count = self.visitor_count.fetch_add(1, Ordering::SeqCst);
        self.send_message("main", &format!("总访客数 {count}"), 0);

        // 发送ID回去
        id
    }
}

/// Disconnect消息处理器
impl Handler<Disconnect> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        log_info!("有人断开连接");

        let mut rooms: Vec<String> = Vec::new();

        // 移除地址
        if self.sessions.remove(&msg.id).is_some() {
            // 从所有房间中移除会话
            for (name, sessions) in &mut self.rooms {
                if sessions.remove(&msg.id) {
                    rooms.push(name.to_owned());
                }
            }
        }

        // 从数据库中删除会话
        if let Some(session_id) = self.id_to_session_id.remove(&msg.id) {
            if let Err(err) = self.db.remove_session(&session_id) {
                log_error!("Failed to remove session: {}", err);
            }
        }

        // 移除用户名映射
        self.id_to_username.remove(&msg.id);

        // 向其他用户发送消息
        for room in rooms {
            self.send_message(&room, "有人断开连接", 0);
        }
    }
}

/// Message消息处理器
impl Handler<ClientMessage> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: ClientMessage, _: &mut Context<Self>) {
        self.send_message(&msg.room, msg.msg.as_str(), msg.id);
    }
}

/// `ListRooms`消息处理器
impl Handler<ListRooms> for ChatServer {
    type Result = MessageResult<ListRooms>;

    fn handle(&mut self, _: ListRooms, _: &mut Context<Self>) -> Self::Result {
        let mut rooms = Vec::new();

        for key in self.rooms.keys() {
            rooms.push(key.to_owned())
        }

        MessageResult(rooms)
    }
}

/// 设置用户名消息处理器
impl Handler<SetUsername> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: SetUsername, _: &mut Context<Self>) {
        let SetUsername { id, username } = msg;

        // 更新内部映射
        self.id_to_username.insert(id, username.clone());

        // 更新数据库中的会话信息
        if let Some(session_id) = self.id_to_session_id.get(&id) {
            // 获取当前房间
            let mut current_room = None;
            for (room, sessions) in &self.rooms {
                if sessions.contains(&id) {
                    current_room = Some(room);
                    break;
                }
            }

            if let Err(err) = self.db.save_session(
                session_id,
                Some(&username),
                current_room.map(|r| r.as_str()),
            ) {
                log_error!("Failed to update session username: {}", err);
            }
        }
    }
}

/// 处理私有消息
impl Handler<PrivateMessage> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: PrivateMessage, _: &mut Context<Self>) {
        let PrivateMessage {
            sender_id,
            receiver_username,
            msg: content,
        } = msg;

        // 获取发送者用户名
        let sender_username = self
            .id_to_username
            .get(&sender_id)
            .map(|name| name.clone())
            .unwrap_or_else(|| "匿名".to_string());

        // 查找接收者的会话ID
        let mut found = false;
        for (id, username) in &self.id_to_username {
            if username == &receiver_username {
                // 找到接收者，发送私信
                if let Some(addr) = self.sessions.get(id) {
                    let formatted_message = format!("[私信] {}: {}", sender_username, content);
                    addr.do_send(Message(formatted_message));
                    found = true;
                }
                break;
            }
        }

        // 如果找不到接收者，通知发送者
        if !found {
            if let Some(sender_addr) = self.sessions.get(&sender_id) {
                sender_addr.do_send(Message(format!("找不到用户: {}", receiver_username)));
            }
            log_info!("找不到用户 {} 发送私信", receiver_username);
            return;
        }

        // 通知发送者私信已发送
        if let Some(sender_addr) = self.sessions.get(&sender_id) {
            sender_addr.do_send(Message(format!(
                "[发送给 {}] {}",
                receiver_username, content
            )));
        }

        // 保存私信到数据库
        if let Some(session_id) = self.id_to_session_id.get(&sender_id) {
            // 对于私信，房间字段可以设置为特殊值表示是私信
            let private_room = format!("private:{}->{}", sender_username, receiver_username);
            if let Err(err) =
                self.db
                    .save_message(session_id, &sender_username, Some(&private_room), &content)
            {
                log_error!("保存私信失败: {}", err);
            }
        }

        log_info!(
            "用户 {} 向 {} 发送了私信",
            sender_username,
            receiver_username
        );
    }
}

/// 处理历史消息请求
impl Handler<GetHistoryMessages> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: GetHistoryMessages, _: &mut Context<Self>) {
        let GetHistoryMessages {
            room,
            limit,
            requester_id,
        } = msg;

        // 从数据库加载历史消息
        match self.db.load_messages(Some(&room), limit) {
            Ok(messages) => {
                log_info!("为房间 {} 加载了 {} 条历史消息", room, messages.len());

                // 发送历史消息给请求者
                if let Some(addr) = self.sessions.get(&requester_id) {
                    // 先发送历史消息开始标记
                    addr.do_send(Message("--- 历史消息开始 ---".to_owned()));

                    // 发送每条历史消息
                    for (_, username, _, content, timestamp) in messages {
                        let formatted_message =
                            format!("[{}] {}: {}", timestamp, username, content);
                        addr.do_send(Message(formatted_message));
                    }

                    // 发送历史消息结束标记
                    addr.do_send(Message("--- 历史消息结束 ---".to_owned()));
                }
            }
            Err(err) => {
                log_error!("加载历史消息失败: {}", err);

                // 发送错误消息
                if let Some(addr) = self.sessions.get(&requester_id) {
                    addr.do_send(Message("获取历史消息失败".to_owned()));
                }
            }
        }
    }
}

/// 加入房间，向旧房间发送断开连接消息
/// 向新房间发送加入消息
impl Handler<Join> for ChatServer {
    type Result = ();

    fn handle(&mut self, msg: Join, _: &mut Context<Self>) {
        let Join { id, name } = msg;
        let mut rooms = Vec::new();

        // remove session from all rooms
        for (n, sessions) in &mut self.rooms {
            if sessions.remove(&id) {
                rooms.push(n.to_owned());
            }
        }
        // 向其他用户发送消息
        for room in rooms {
            self.send_message(&room, "有人断开连接", 0);
        }

        self.rooms.entry(name.clone()).or_default().insert(id);

        // 更新数据库中的会话房间信息
        if let Some(session_id) = self.id_to_session_id.get(&id) {
            let username = self.id_to_username.get(&id).map(|s| s.as_str());
            if let Err(err) = self.db.save_session(session_id, username, Some(&name)) {
                log_error!("Failed to update session room: {}", err);
            }
        }

        self.send_message(&name, "有人已连接", id);
    }
}

/// 心跳ping发送频率
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// 客户端无响应导致超时的时间
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

// Actor 类型已集中到 actors.rs
impl WsChatSession {
    /// 辅助方法，每5秒（HEARTBEAT_INTERVAL）向客户端发送ping
    ///
    /// 此方法还检查来自客户端的心跳
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // 检查客户端心跳
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // 心跳超时
                log_info!("WebSocket客户端心跳失败，正在断开连接！");

                // 通知聊天服务器
                act.addr.do_send(Disconnect { id: act.id });

                // 停止actor
                ctx.stop();

                // 不要尝试发送ping
                return;
            }

            ctx.ping(b"");
        });
    }
}

impl Actor for WsChatSession {
    type Context = ws::WebsocketContext<Self>;

    /// 在actor启动时调用此方法
    /// 我们向ChatServer注册WebSocket会话
    fn started(&mut self, ctx: &mut Self::Context) {
        // 会话启动时开始心跳过程
        self.hb(ctx);

        // 在聊天服务器中注册自己。`AsyncContext::wait` 在上下文中注册
        // future，但上下文会等待这个future解析
        // 然后再处理任何其他事件
        // HttpContext::state() 是 WsChatSessionState 的实例，状态在
        // 应用程序的所有路由之间共享
        let addr = ctx.address();
        self.addr
            .send(Connect {
                addr: addr.recipient(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
                    Ok(res) => act.id = res,
                    // 聊天服务器有问题
                    _ => ctx.stop(),
                }
                fut::ready(())
            })
            .wait(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        // 通知聊天服务器
        self.addr.do_send(Disconnect { id: self.id });
        Running::Stop
    }
}

/// 处理来自聊天服务器的消息，我们简单地将其发送到对等WebSocket
impl Handler<Message> for WsChatSession {
    type Result = ();

    fn handle(&mut self, msg: Message, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

/// WebSocket消息处理器
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsChatSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        let msg = match msg {
            Err(_) => {
                ctx.stop();
                return;
            }
            Ok(msg) => msg,
        };

        log_debug!("WEBSOCKET消息: {msg:?}");
        match msg {
            ws::Message::Ping(msg) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.hb = Instant::now();
            }
            ws::Message::Text(text) => {
                let m = text.trim();
                // we check for /sss type of messages
                if m.starts_with('/') {
                    let v: Vec<&str> = m.splitn(2, ' ').collect();
                    match v[0] {
                        "/list" => {
                            // 向聊天服务器发送ListRooms消息并等待
                            // 响应
                            log_info!("列出房间");
                            self.addr
                                .send(ListRooms)
                                .into_actor(self)
                                .then(|res, _, ctx| {
                                    match res {
                                        Ok(rooms) => {
                                            for room in rooms {
                                                ctx.text(room);
                                            }
                                        }
                                        _ => log_info!("出现错误"),
                                    }
                                    fut::ready(())
                                })
                                .wait(ctx)
                            // .wait(ctx) pauses all events in context,
                            // so actor wont receive any new messages until it get list
                            // of rooms back
                        }
                        "/join" => {
                            if v.len() == 2 {
                                v[1].clone_into(&mut self.room);
                                self.addr.do_send(Join {
                                    id: self.id,
                                    name: self.room.clone(),
                                });

                                ctx.text("joined");
                            } else {
                                ctx.text("!!! 需要房间名称");
                            }
                        }
                        "/name" => {
                            if v.len() == 2 {
                                self.name = Some(v[1].to_owned());
                                // 通知服务器用户名已设置
                                if let Err(err) = self.addr.try_send(SetUsername {
                                    id: self.id,
                                    username: v[1].to_owned(),
                                }) {
                                    log_error!("Failed to send SetUsername message: {}", err);
                                }
                            } else {
                                ctx.text("!!! 需要名称");
                            }
                        }
                        "/history" => {
                            // 处理历史消息请求
                            let (room, limit) = if v.len() >= 3 {
                                (v[1].to_owned(), v[2].parse().unwrap_or(50) as i32)
                            } else if v.len() == 2 {
                                if let Ok(l) = v[1].parse::<i32>() {
                                    (self.room.clone(), l)
                                } else {
                                    (v[1].to_owned(), 50)
                                }
                            } else {
                                (self.room.clone(), 50)
                            };

                            log_info!("请求房间 {} 的历史消息，限制 {} 条", room, limit);

                            // 创建一个新的数据库引用，因为不能在Actor的上下文中阻塞
                            let db = self.addr.try_send(GetHistoryMessages {
                                room: room.to_owned(),
                                limit,
                                requester_id: self.id,
                            });

                            if let Err(err) = db {
                                log_error!("Failed to request history messages: {}", err);
                                ctx.text("获取历史消息失败");
                            }
                        }
                        "/to" => {
                            // 处理私信命令 /to username message
                            if v.len() >= 2 {
                                // 分割命令以获取用户名和消息内容
                                let parts: Vec<&str> = v[1].splitn(2, ' ').collect();
                                if parts.len() >= 2 {
                                    let receiver_username = parts[0].to_owned();
                                    let message_content = parts[1].to_owned();

                                    // 发送私有消息到服务器
                                    if let Err(err) = self.addr.try_send(PrivateMessage {
                                        sender_id: self.id,
                                        receiver_username,
                                        msg: message_content,
                                    }) {
                                        log_error!("Failed to send private message: {}", err);
                                        ctx.text("发送私信失败");
                                    }
                                } else {
                                    ctx.text("!!! 格式错误: /to 用户名 消息内容");
                                }
                            } else {
                                ctx.text("!!! 需要用户名和消息内容");
                            }
                        }
                        _ => ctx.text(format!("!!! 未知命令: {m:?}")),
                    }
                } else {
                    // 支持JSON媒体消息：如果是JSON且type为media，直接转发原始内容
                    let is_media_json = if m.starts_with('{') {
                        match serde_json::from_str::<serde_json::Value>(m) {
                            Ok(v) => v
                                .get("type")
                                .and_then(|t| t.as_str())
                                .map(|s| s == "media")
                                .unwrap_or(false),
                            Err(_) => false,
                        }
                    } else {
                        false
                    };

                    if is_media_json {
                        self.addr.do_send(ClientMessage {
                            id: self.id,
                            msg: m.to_owned(),
                            room: self.room.clone(),
                        })
                    } else {
                        let msg = if let Some(ref name) = self.name {
                            format!("{name}: {m}")
                        } else {
                            m.to_owned()
                        };
                        // 向聊天服务器发送消息
                        self.addr.do_send(ClientMessage {
                            id: self.id,
                            msg,
                            room: self.room.clone(),
                        })
                    }
                }
            }
            ws::Message::Binary(_) => log_info!("意外的二进制数据"),
            ws::Message::Close(reason) => {
                ctx.close(reason);
                ctx.stop();
            }
            ws::Message::Continuation(_) => {
                ctx.stop();
            }
            ws::Message::Nop => (),
        }
    }
}
