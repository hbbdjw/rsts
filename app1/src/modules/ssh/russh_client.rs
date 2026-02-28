use anyhow::Result;
use log::{debug, error, info};
use russh::client::{self};
use russh::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, mpsc};

#[allow(dead_code)]
pub struct RusshClient {
    session: Option<client::Handle<Client>>,
    channel: Option<Arc<Mutex<russh::Channel<russh::client::Msg>>>>,
    input_tx: Option<mpsc::UnboundedSender<Vec<u8>>>,
    host: String,
    port: u16,
    username: String,
    password: String,
}

#[allow(dead_code)]
impl RusshClient {
    pub fn new(host: String, port: u16, username: String, password: String) -> Self {
        Self {
            session: None,
            channel: None,
            input_tx: None,
            host,
            port,
            username,
            password,
        }
    }

    pub async fn connect(&mut self) -> Result<()> {
        let config = client::Config {
            inactivity_timeout: Some(Duration::from_secs(300)),
            ..Default::default()
        };
        let config = Arc::new(config);
        let sh = Client {};

        info!("连接到SSH服务器: {}:{}", self.host, self.port);
        let mut session = client::connect(config, (self.host.as_str(), self.port), sh).await?;

        // 使用密码认证
        let auth_res = session
            .authenticate_password(self.username.clone(), self.password.clone())
            .await?;

        if auth_res != client::AuthResult::Success {
            return Err(anyhow::anyhow!("SSH认证失败"));
        }

        info!("SSH认证成功");
        self.session = Some(session);
        Ok(())
    }

    pub async fn create_pty_session(
        &mut self,
        width: Option<u32>,
        height: Option<u32>,
    ) -> Result<()> {
        if let Some(ref mut session) = self.session {
            let channel = session.channel_open_session().await?;

            // 请求PTY，使用传入尺寸，否则默认80x24
            let cols = width.unwrap_or(80);
            let rows = height.unwrap_or(24);
            channel
                .request_pty(false, "xterm-256color", cols, rows, 0, 0, &[])
                .await?;

            // 启动shell
            channel.request_shell(false).await?;

            self.channel = Some(Arc::new(Mutex::new(channel)));
            info!("PTY会话创建成功");
            Ok(())
        } else {
            Err(anyhow::anyhow!("SSH会话未建立"))
        }
    }

    pub async fn write_to_pty(&self, data: &[u8]) -> Result<()> {
        info!(
            "RusshClient: 接收到写入请求，数据长度: {}字节，数据内容: {:?}",
            data.len(),
            String::from_utf8_lossy(data)
        );

        // 优先通过输入队列发送，避免与读取器互相阻塞
        if let Some(ref tx) = self.input_tx {
            info!("RusshClient: 通过输入队列发送数据");
            tx.send(data.to_vec())
                .map_err(|_| anyhow::anyhow!("输入通道发送失败"))?;
            return Ok(());
        }

        // 回退：直接写入通道（不推荐，可能与读取器竞争锁）
        if let Some(ref channel) = self.channel {
            info!("RusshClient: 直接写入通道");
            let channel = channel.lock().await;
            channel.data(data).await?;
            info!(
                "RusshClient: 向PTY写入数据(直写)成功: {:?}",
                String::from_utf8_lossy(data)
            );
            Ok(())
        } else {
            error!("RusshClient: PTY会话未建立，无法写入数据");
            Err(anyhow::anyhow!("PTY会话未建立"))
        }
    }

    /// 启动单线程的PTY IO循环，避免读写锁竞争导致的阻塞
    pub async fn start_pty_io(&mut self, tx: mpsc::UnboundedSender<Vec<u8>>) -> Result<()> {
        if let Some(ref channel_arc) = self.channel {
            let channel_arc = channel_arc.clone();
            let (in_tx, mut in_rx) = mpsc::unbounded_channel::<Vec<u8>>();
            // 保存输入发送端
            self.input_tx = Some(in_tx.clone());

            tokio::spawn(async move {
                // 独占锁一次，然后在该任务内协调读写
                let mut channel = channel_arc.lock().await;
                loop {
                    tokio::select! {
                        // 处理写入请求
                        Some(data) = in_rx.recv() => {
                            match channel.data(&data[..]).await {
                                Ok(_) => debug!("向PTY写入数据: {:?}", String::from_utf8_lossy(&data)),
                                Err(e) => error!("向PTY写入数据失败: {}", e),
                            }
                        },
                        // 处理读取数据
                        msg = channel.wait() => {
                            match msg {
                                Some(russh::ChannelMsg::Data { data }) => {
                                    debug!("从PTY读取数据: {:?}", String::from_utf8_lossy(&data));
                                    if tx.send(data.to_vec()).is_err() {
                                        error!("发送PTY数据失败");
                                        break;
                                    }
                                }
                                Some(russh::ChannelMsg::Eof) => {
                                    info!("PTY会话结束");
                                    break;
                                }
                                Some(russh::ChannelMsg::Close) => {
                                    info!("PTY通道关闭");
                                    break;
                                }
                                Some(russh::ChannelMsg::ExtendedData { data, ext: _ }) => {
                                    debug!("从PTY读取扩展数据: {:?}", String::from_utf8_lossy(&data));
                                    if tx.send(data.to_vec()).is_err() {
                                        error!("发送PTY扩展数据失败");
                                        break;
                                    }
                                }
                                _ => {
                                    // 其他消息类型，继续循环
                                    continue;
                                }
                            }
                        }
                    }
                }
            });

            Ok(())
        } else {
            Err(anyhow::anyhow!("PTY会话未建立"))
        }
    }

    pub async fn resize_pty(&self, width: u32, height: u32) -> Result<()> {
        if let Some(ref channel) = self.channel {
            let channel = channel.lock().await;
            channel.window_change(width, height, 0, 0).await?;
            debug!("PTY窗口大小调整为: {}x{}", width, height);
            Ok(())
        } else {
            Err(anyhow::anyhow!("PTY会话未建立"))
        }
    }

    pub async fn close(&mut self) -> Result<()> {
        if let Some(ref channel) = self.channel {
            let channel = channel.lock().await;
            channel.close().await?;
        }

        if let Some(ref mut session) = self.session {
            session
                .disconnect(Disconnect::ByApplication, "", "")
                .await?;
        }

        info!("SSH连接已关闭");
        Ok(())
    }
}

struct Client {}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        // 在生产环境中应该验证服务器密钥
        Ok(true)
    }
}
