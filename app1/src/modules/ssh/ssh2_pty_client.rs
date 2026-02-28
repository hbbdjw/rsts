use anyhow::Result;
use log::{debug, error, info};
use ssh2::Channel as Ssh2Channel;
use ssh2::Session as Ssh2Session;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::time::Duration;

pub struct Ssh2PtyClient {
    session: Option<Ssh2Session>,
    channel: Option<Ssh2Channel>,
    command_tx: Option<mpsc::Sender<PtyCommand>>,
    host: String,
    port: u16,
    username: String,
    password: String,
}

enum PtyCommand {
    Write(Vec<u8>),
    Resize { width: u32, height: u32 },
    Close,
}

fn is_would_block(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::WouldBlock
}

impl Ssh2PtyClient {
    pub fn new(host: String, port: u16, username: String, password: String) -> Self {
        Self {
            session: None,
            channel: None,
            command_tx: None,
            host,
            port,
            username,
            password,
        }
    }

    pub async fn connect(&mut self) -> Result<()> {
        let addr = format!("{}:{}", self.host, self.port);
        let tcp = TcpStream::connect(addr)?;
        tcp.set_read_timeout(Some(Duration::from_secs(30)))?;
        tcp.set_write_timeout(Some(Duration::from_secs(30)))?;
        let mut sess = Ssh2Session::new()?;
        sess.set_tcp_stream(tcp);
        sess.handshake()?;
        sess.set_blocking(true);
        sess.userauth_password(&self.username, &self.password)?;
        if !sess.authenticated() {
            return Err(anyhow::anyhow!("auth failed"));
        }
        info!("ssh2 connected");
        self.session = Some(sess);
        Ok(())
    }

    pub async fn create_pty_session(
        &mut self,
        width: Option<u32>,
        height: Option<u32>,
    ) -> Result<()> {
        let sess = self
            .session
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no session"))?;
        let mut ch = sess.channel_session()?;
        let cols = width.unwrap_or(80);
        let rows = height.unwrap_or(24);
        ch.request_pty("xterm-256color", None, Some((cols, rows, 0, 0)))?;
        ch.shell()?;
        self.channel = Some(ch);
        info!("ssh2 pty created");
        Ok(())
    }

    pub async fn start_pty_io(
        &mut self,
        tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    ) -> Result<()> {
        if let Some(sess) = self.session.as_mut() {
            sess.set_blocking(false);
        }
        let mut channel = self
            .channel
            .take()
            .ok_or_else(|| anyhow::anyhow!("no channel"))?;
        let (command_tx, command_rx) = mpsc::channel::<PtyCommand>();
        self.command_tx = Some(command_tx);

        tokio::task::spawn_blocking(move || {
            info!("ssh2 pty io loop started");
            let mut buf = [0u8; 4096];
            let mut pending_write: Option<(Vec<u8>, usize)> = None;

            loop {
                if pending_write.is_none() {
                    loop {
                        match command_rx.try_recv() {
                            Ok(PtyCommand::Write(data)) => {
                                pending_write = Some((data, 0));
                                break;
                            }
                            Ok(PtyCommand::Resize { width, height }) => {
                                let _ = channel.request_pty_size(width, height, None, None);
                            }
                            Ok(PtyCommand::Close) => {
                                let _ = channel.close();
                                info!("ssh2 pty io loop ended");
                                return Ok::<(), anyhow::Error>(());
                            }
                            Err(mpsc::TryRecvError::Empty) => break,
                            Err(mpsc::TryRecvError::Disconnected) => {
                                let _ = channel.close();
                                info!("ssh2 pty io loop ended");
                                return Ok::<(), anyhow::Error>(());
                            }
                        }
                    }
                }

                if let Some((data, offset)) = pending_write.take() {
                    if offset < data.len() {
                        match channel.write(&data[offset..]) {
                            Ok(n) => {
                                let next = offset.saturating_add(n);
                                if next < data.len() {
                                    pending_write = Some((data, next));
                                    std::thread::sleep(Duration::from_millis(1));
                                    continue;
                                }
                                let _ = channel.flush();
                            }
                            Err(e) => {
                                if is_would_block(&e) {
                                    pending_write = Some((data, offset));
                                    std::thread::sleep(Duration::from_millis(10));
                                    continue;
                                }
                                error!("ssh2 write error: {}", e);
                                break;
                            }
                        }
                    }
                }

                match channel.read(&mut buf) {
                    Ok(n) => {
                        if n == 0 {
                            info!("ssh2 channel closed, no more data to read");
                            break;
                        }

                        let data = buf[..n].to_vec();
                        debug!("ssh2 read {} bytes", n);
                        if tx.send(data).is_err() {
                            error!("failed to send data to output channel");
                            break;
                        }
                    }
                    Err(e) => {
                        if is_would_block(&e) {
                            std::thread::sleep(Duration::from_millis(10));
                            continue;
                        }
                        error!("ssh2 read error: {}", e);
                        break;
                    }
                }
            }

            info!("ssh2 pty io loop ended");
            Ok::<(), anyhow::Error>(())
        });

        Ok(())
    }

    pub async fn write_to_pty(&self, data: &[u8]) -> Result<()> {
        let tx = self
            .command_tx
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no pty io loop"))?;
        tx.send(PtyCommand::Write(data.to_vec()))
            .map_err(|_| anyhow::anyhow!("pty io loop closed"))?;
        Ok(())
    }

    pub async fn resize_pty(&self, width: u32, height: u32) -> Result<()> {
        let tx = self
            .command_tx
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no pty io loop"))?;
        tx.send(PtyCommand::Resize { width, height })
            .map_err(|_| anyhow::anyhow!("pty io loop closed"))?;
        Ok(())
    }

    pub async fn close(&mut self) -> Result<()> {
        if let Some(tx) = self.command_tx.take() {
            let _ = tx.send(PtyCommand::Close);
        }
        if let Some(sess) = self.session.take() {
            let _ = sess.disconnect(None, "", None);
        }
        Ok(())
    }
}
