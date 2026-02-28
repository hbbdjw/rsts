use actix::Addr;
use actix::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, atomic::AtomicBool, atomic::AtomicUsize};
use std::time::Instant;
use tokio::sync::Mutex;

use super::database::Database;
use super::models::{AnyPtyClient, Message};
use crate::modules::ssh::SshService;

// 聊天服务器 Actor
#[derive(Debug)]
pub struct ChatServer {
    pub sessions: HashMap<u64, Recipient<Message>>,
    pub rooms: HashMap<String, HashSet<u64>>,
    pub visitor_count: Arc<AtomicUsize>,
    pub db: Arc<Database>,
    pub id_to_session_id: HashMap<u64, String>,
    pub id_to_username: HashMap<u64, String>,
}

// WebSocket 聊天会话 Actor
#[derive(Debug)]
pub struct WsChatSession {
    pub id: u64,
    pub hb: Instant,
    pub room: String,
    pub name: Option<String>,
    pub addr: Addr<ChatServer>,
}

// SSH WebSocket 会话 Actor
pub struct WsSshSession {
    pub hb: Instant,
    pub ssh_service: Arc<Mutex<SshService>>,
    pub ssh_client_id: Option<usize>,
    pub connected_to_ssh: Arc<AtomicBool>,
}

// SSH PTY WebSocket 会话 Actor
pub struct WsSshPtySession {
    pub hb: Instant,
    pub ssh_client: Option<AnyPtyClient>,
    pub connected: bool,
}
