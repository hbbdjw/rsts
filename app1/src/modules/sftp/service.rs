use base64::prelude::*;
use russh::client::{self, Config};
use russh_sftp::client::SftpSession as SftpClient;
use ssh2::MethodType;
use ssh2::Session as Ssh2Session;
use ssh2::Sftp as Ssh2Sftp;
use std::future::Future;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex as TokioMutex;

#[derive(Clone, Debug)]
pub struct SftpCredentials {
    pub hostname: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

#[allow(dead_code)]
pub struct Ssh2SessionImpl {
    pub ssh: Ssh2Session,
    pub sftp: Ssh2Sftp,
}

pub(crate) struct ClientHandler;
impl client::Handler for ClientHandler {
    type Error = russh::Error;
    fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKey,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        async { Ok(true) }
    }
}

#[derive(Clone)]
pub struct RusshSessionImpl {
    pub(crate) session: Arc<client::Handle<ClientHandler>>,
    pub sftp: Arc<SftpClient>,
}

pub enum AnySftpSession {
    Ssh2(Arc<StdMutex<Ssh2SessionImpl>>),
    Russh(RusshSessionImpl),
}

pub struct SftpService {
    sessions: Arc<TokioMutex<Vec<Option<AnySftpSession>>>>,
}

impl SftpService {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(TokioMutex::new(Vec::new())),
        }
    }

    pub async fn create_session(&self, creds: SftpCredentials) -> anyhow::Result<usize> {
        match Self::connect_ssh2(&creds) {
            Ok(ssh2_impl) => {
                let session = AnySftpSession::Ssh2(Arc::new(StdMutex::new(ssh2_impl)));
                return self.store_session(session).await;
            }
            Err(e_ssh2) => {
                log::info!("SSH2 connection failed ({}), trying Russh...", e_ssh2);
                match Self::connect_russh(&creds).await {
                    Ok(russh_impl) => {
                        let session = AnySftpSession::Russh(russh_impl);
                        return self.store_session(session).await;
                    }
                    Err(e_russh) => {
                        return Err(anyhow::anyhow!(
                            "SFTP connection failed. SSH2: {}, Russh: {}",
                            e_ssh2,
                            e_russh
                        ));
                    }
                }
            }
        }
    }

    async fn store_session(&self, session: AnySftpSession) -> anyhow::Result<usize> {
        let mut sessions = self.sessions.lock().await;
        if let Some((idx, slot)) = sessions.iter_mut().enumerate().find(|(_, s)| s.is_none()) {
            *slot = Some(session);
            Ok(idx)
        } else {
            sessions.push(Some(session));
            Ok(sessions.len() - 1)
        }
    }

    fn connect_ssh2(creds: &SftpCredentials) -> anyhow::Result<Ssh2SessionImpl> {
        let addr = format!("{}:{}", creds.hostname, creds.port);
        let tcp = TcpStream::connect(addr)?;
        tcp.set_read_timeout(Some(Duration::from_secs(30)))?;
        tcp.set_write_timeout(Some(Duration::from_secs(30)))?;

        let mut sess = Ssh2Session::new()?;
        sess.set_tcp_stream(tcp);

        let _ = sess.method_pref(
            MethodType::Kex,
            "curve25519-sha256,curve25519-sha256@libssh.org,ecdh-sha2-nistp256,ecdh-sha2-nistp384,ecdh-sha2-nistp521,diffie-hellman-group-exchange-sha256,diffie-hellman-group16-sha512,diffie-hellman-group18-sha512,diffie-hellman-group14-sha256"
        );
        let _ = sess.method_pref(
            MethodType::HostKey,
            "ecdsa-sha2-nistp256,ecdsa-sha2-nistp384,ecdsa-sha2-nistp521,ssh-ed25519,rsa-sha2-512,rsa-sha2-256,ssh-rsa"
        );
        let crypt = "aes128-ctr,aes192-ctr,aes256-ctr,aes128-gcm@openssh.com,aes256-gcm@openssh.com,chacha20-poly1305@openssh.com";
        let _ = sess.method_pref(MethodType::CryptCs, crypt);
        let _ = sess.method_pref(MethodType::CryptSc, crypt);
        let mac = "hmac-sha2-256,hmac-sha2-512,hmac-sha2-256-etm@openssh.com,hmac-sha2-512-etm@openssh.com";
        let _ = sess.method_pref(MethodType::MacCs, mac);
        let _ = sess.method_pref(MethodType::MacSc, mac);

        sess.handshake()?;

        let mut authed = false;
        match sess.userauth_password(&creds.username, &creds.password) {
            Ok(()) => authed = sess.authenticated(),
            Err(_) => {}
        }

        if !authed {
            struct KI {
                pwd: String,
            }
            impl ssh2::KeyboardInteractivePrompt for KI {
                fn prompt<'a>(
                    &mut self,
                    _username: &str,
                    _instructions: &str,
                    prompts: &[ssh2::Prompt<'a>],
                ) -> Vec<String> {
                    prompts.iter().map(|_| self.pwd.clone()).collect()
                }
            }
            let mut ki = KI {
                pwd: creds.password.clone(),
            };
            if sess
                .userauth_keyboard_interactive(&creds.username, &mut ki)
                .is_ok()
            {
                authed = sess.authenticated();
            }
        }

        if !authed {
            return Err(anyhow::anyhow!("SSH2 Authentication failed"));
        }

        let sftp = sess.sftp()?;
        Ok(Ssh2SessionImpl { ssh: sess, sftp })
    }

    async fn connect_russh(creds: &SftpCredentials) -> anyhow::Result<RusshSessionImpl> {
        let config = Config {
            inactivity_timeout: None,
            ..Default::default()
        };
        let config = Arc::new(config);
        let sh = ClientHandler;

        let mut session =
            client::connect(config, (creds.hostname.as_str(), creds.port), sh).await?;

        let auth_res = session
            .authenticate_password(creds.username.clone(), creds.password.clone())
            .await?;
        if !matches!(auth_res, client::AuthResult::Success) {
            return Err(anyhow::anyhow!("Russh Authentication failed"));
        }

        let channel = session.channel_open_session().await?;
        channel.request_subsystem(true, "sftp").await?;
        let sftp = SftpClient::new(channel.into_stream()).await?;

        Ok(RusshSessionImpl {
            session: Arc::new(session),
            sftp: Arc::new(sftp),
        })
    }

    pub async fn get_session(&self, session_id: usize) -> anyhow::Result<SftpSessionGuard> {
        let sessions = self.sessions.lock().await;
        if let Some(Some(_)) = sessions.get(session_id) {
            // Valid
        } else {
            return Err(anyhow::anyhow!("无效的SFTP会话ID"));
        }
        drop(sessions);
        Ok(SftpSessionGuard {
            service: self.sessions.clone(),
            id: session_id,
        })
    }

    #[allow(dead_code)]
    pub async fn close_session(&self, session_id: usize) -> anyhow::Result<()> {
        let mut sessions = self.sessions.lock().await;
        if let Some(slot) = sessions.get_mut(session_id) {
            *slot = None;
            Ok(())
        } else {
            Err(anyhow::anyhow!("无效的SFTP会话ID"))
        }
    }
}

pub struct SftpSessionGuard {
    service: Arc<TokioMutex<Vec<Option<AnySftpSession>>>>,
    id: usize,
}

impl SftpSessionGuard {
    async fn get_session_variant(&self) -> anyhow::Result<AnySftpSession> {
        let sessions = self.service.lock().await;
        match sessions.get(self.id) {
            Some(Some(AnySftpSession::Ssh2(s))) => Ok(AnySftpSession::Ssh2(s.clone())),
            Some(Some(AnySftpSession::Russh(s))) => Ok(AnySftpSession::Russh(RusshSessionImpl {
                session: s.session.clone(),
                sftp: s.sftp.clone(),
            })),
            _ => Err(anyhow::anyhow!("会话不存在")),
        }
    }

    pub async fn list(&self, path: &str) -> anyhow::Result<Vec<FileEntry>> {
        match self.get_session_variant().await? {
            AnySftpSession::Ssh2(wrapper) => {
                let path = path.to_string();
                tokio::task::spawn_blocking(move || {
                    let guard = wrapper
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Lock poisoned"))?;
                    let mut out = Vec::new();
                    let entries = guard.sftp.readdir(std::path::Path::new(&path))?;
                    for (name_path, stat) in entries {
                        let name_os = name_path.file_name().unwrap_or_default();
                        let name_str = name_os.to_string_lossy().to_string();
                        if name_str.is_empty() || name_str == "." || name_str == ".." {
                            continue;
                        }
                        let is_dir = stat.file_type().is_dir();
                        let size = stat.size.unwrap_or(0);
                        let permissions = stat.perm.unwrap_or(0);
                        let mtime = stat.mtime.unwrap_or(0);
                        let path_rel = if path.ends_with('/') {
                            format!("{}{}", path, name_str)
                        } else {
                            format!("{}/{}", path, name_str)
                        };
                        out.push(FileEntry {
                            name: name_str,
                            path: path_rel,
                            size,
                            is_dir,
                            permissions,
                            mtime,
                        });
                    }
                    Ok(out)
                })
                .await?
            }
            AnySftpSession::Russh(s) => {
                let entries = s.sftp.read_dir(path).await?;
                let mut out = Vec::new();
                for entry in entries {
                    let name = entry.file_name();
                    if name == "." || name == ".." {
                        continue;
                    }

                    let attrs = entry.metadata();
                    let size = attrs.size.unwrap_or(0);
                    let permissions = attrs.permissions.unwrap_or(0);
                    let mtime = attrs.mtime.unwrap_or(0) as u64;
                    let is_dir = attrs.is_dir();

                    let path_rel = if path.ends_with('/') {
                        format!("{}{}", path, name)
                    } else {
                        format!("{}/{}", path, name)
                    };

                    out.push(FileEntry {
                        name,
                        path: path_rel,
                        size,
                        is_dir,
                        permissions,
                        mtime,
                    });
                }
                Ok(out)
            }
        }
    }

    pub async fn read_text(&self, path: &str) -> anyhow::Result<String> {
        match self.get_session_variant().await? {
            AnySftpSession::Ssh2(wrapper) => {
                let path = path.to_string();
                tokio::task::spawn_blocking(move || {
                    let guard = wrapper
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Lock poisoned"))?;
                    let mut file = guard.sftp.open(std::path::Path::new(&path))?;
                    let mut buf = Vec::new();
                    use std::io::Read;
                    file.read_to_end(&mut buf)?;
                    let s = String::from_utf8(buf).unwrap_or_else(|_| String::from("(非文本文件)"));
                    Ok(s)
                })
                .await?
            }
            AnySftpSession::Russh(s) => {
                let mut file = s.sftp.open(path).await?;
                let mut buf = Vec::new();
                file.read_to_end(&mut buf).await?;
                let s = String::from_utf8(buf).unwrap_or_else(|_| String::from("(非文本文件)"));
                Ok(s)
            }
        }
    }

    pub async fn write_text(&self, path: &str, content: &str) -> anyhow::Result<()> {
        match self.get_session_variant().await? {
            AnySftpSession::Ssh2(wrapper) => {
                let path = path.to_string();
                let content = content.to_string();
                tokio::task::spawn_blocking(move || {
                    let guard = wrapper
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Lock poisoned"))?;
                    let mut file = guard.sftp.create(std::path::Path::new(&path))?;
                    use std::io::Write;
                    file.write_all(content.as_bytes())?;
                    Ok(())
                })
                .await?
            }
            AnySftpSession::Russh(s) => {
                let mut file = s.sftp.create(path).await?;
                file.write_all(content.as_bytes()).await?;
                Ok(())
            }
        }
    }

    pub async fn delete(&self, path: &str) -> anyhow::Result<()> {
        match self.get_session_variant().await? {
            AnySftpSession::Ssh2(wrapper) => {
                let path = path.to_string();
                tokio::task::spawn_blocking(move || {
                    let guard = wrapper
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Lock poisoned"))?;
                    let p = std::path::Path::new(&path);
                    if let Err(_) = guard.sftp.unlink(p) {
                        guard.sftp.rmdir(p)?;
                    }
                    Ok(())
                })
                .await?
            }
            AnySftpSession::Russh(s) => {
                if let Err(_) = s.sftp.remove_file(path).await {
                    s.sftp.remove_dir(path).await?;
                }
                Ok(())
            }
        }
    }

    pub async fn rename(&self, from: &str, to_name: &str) -> anyhow::Result<()> {
        match self.get_session_variant().await? {
            AnySftpSession::Ssh2(wrapper) => {
                let from = from.to_string();
                let to_name = to_name.to_string();
                tokio::task::spawn_blocking(move || {
                    let guard = wrapper
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Lock poisoned"))?;
                    let from_path = std::path::Path::new(&from);
                    let parent = from_path.parent().unwrap_or(std::path::Path::new("/"));
                    let to_path = parent.join(&to_name);
                    guard.sftp.rename(from_path, &to_path, None)?;
                    Ok(())
                })
                .await?
            }
            AnySftpSession::Russh(s) => {
                // Russh expects full path for rename?
                let from_path = std::path::Path::new(from);
                let parent = from_path.parent().unwrap_or(std::path::Path::new("/"));
                let to_path = parent.join(to_name);
                let to_path_str = to_path.to_str().unwrap_or_default();
                s.sftp.rename(from, to_path_str).await?;
                Ok(())
            }
        }
    }

    pub async fn upload_base64(
        &self,
        dest_dir: &str,
        filename: &str,
        base64: &str,
    ) -> anyhow::Result<()> {
        match self.get_session_variant().await? {
            AnySftpSession::Ssh2(wrapper) => {
                let dest_dir = dest_dir.to_string();
                let filename = filename.to_string();
                let base64 = base64.to_string();
                tokio::task::spawn_blocking(move || {
                    let guard = wrapper
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Lock poisoned"))?;
                    let decoded = BASE64_STANDARD.decode(base64)?;
                    let dest_path = if dest_dir.ends_with('/') {
                        format!("{}{}", dest_dir, filename)
                    } else {
                        format!("{}/{}", dest_dir, filename)
                    };
                    let dest_path = std::path::Path::new(&dest_path);
                    let mut file = guard.sftp.create(dest_path)?;
                    use std::io::Write;
                    file.write_all(&decoded)?;
                    Ok(())
                })
                .await?
            }
            AnySftpSession::Russh(s) => {
                let decoded = BASE64_STANDARD.decode(base64)?;
                let dest_path = if dest_dir.ends_with('/') {
                    format!("{}{}", dest_dir, filename)
                } else {
                    format!("{}/{}", dest_dir, filename)
                };
                let mut file = s.sftp.create(dest_path).await?;
                file.write_all(&decoded).await?;
                Ok(())
            }
        }
    }

    pub async fn download(&self, path: &str) -> anyhow::Result<Vec<u8>> {
        match self.get_session_variant().await? {
            AnySftpSession::Ssh2(wrapper) => {
                let path = path.to_string();
                tokio::task::spawn_blocking(move || {
                    let guard = wrapper
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Lock poisoned"))?;
                    let mut file = guard.sftp.open(std::path::Path::new(&path))?;
                    let mut buf = Vec::new();
                    use std::io::Read;
                    file.read_to_end(&mut buf)?;
                    Ok(buf)
                })
                .await?
            }
            AnySftpSession::Russh(s) => {
                let mut file = s.sftp.open(path).await?;
                let mut buf = Vec::new();
                file.read_to_end(&mut buf).await?;
                Ok(buf)
            }
        }
    }

    pub async fn mkdir(&self, path: &str) -> anyhow::Result<()> {
        match self.get_session_variant().await? {
            AnySftpSession::Ssh2(wrapper) => {
                let path = path.to_string();
                tokio::task::spawn_blocking(move || {
                    let guard = wrapper
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Lock poisoned"))?;
                    guard.sftp.mkdir(std::path::Path::new(&path), 0o755)?;
                    Ok(())
                })
                .await?
            }
            AnySftpSession::Russh(s) => {
                s.sftp.create_dir(path).await?;
                Ok(())
            }
        }
    }

    pub async fn chmod(&self, path: &str, mode: i32) -> anyhow::Result<()> {
        match self.get_session_variant().await? {
            AnySftpSession::Ssh2(wrapper) => {
                let path = path.to_string();
                tokio::task::spawn_blocking(move || {
                    let guard = wrapper
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Lock poisoned"))?;
                    let mut stat = guard.sftp.stat(std::path::Path::new(&path))?;
                    stat.perm = Some(mode as u32);
                    guard.sftp.setstat(std::path::Path::new(&path), stat)?;
                    Ok(())
                })
                .await?
            }
            AnySftpSession::Russh(s) => {
                // Russh sftp set_stat
                use russh_sftp::protocol::FileAttributes;
                let mut attrs = FileAttributes::default();
                attrs.permissions = Some(mode as u32);
                s.sftp.set_metadata(path, attrs).await?;
                Ok(())
            }
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    pub permissions: u32,
    pub mtime: u64,
}
