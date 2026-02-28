use ssh2::Session as Ssh2Session;
use ssh2::Sftp as Ssh2Sftp;
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct SftpCredentials {
    pub hostname: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

pub struct SftpSession {
    pub ssh: Ssh2Session,
    pub sftp: Ssh2Sftp,
}

pub struct SftpService {
    sessions: Arc<Mutex<Vec<Option<SftpSession>>>>,
}

impl SftpService {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn create_session(&self, creds: SftpCredentials) -> anyhow::Result<usize> {
        // libssh2使用阻塞IO，这里在异步函数中直接调用，规模较小时可接受；如需更优可spawn_blocking
        let addr = format!("{}:{}", creds.hostname, creds.port);
        let tcp = TcpStream::connect(addr)?;
        tcp.set_read_timeout(Some(Duration::from_secs(30)))?;
        tcp.set_write_timeout(Some(Duration::from_secs(30)))?;

        let mut sess = Ssh2Session::new()?;
        sess.set_tcp_stream(tcp);
        sess.handshake()?;

        // 1) 尝试密码认证
        let mut authed = false;
        match sess.userauth_password(&creds.username, &creds.password) {
            Ok(()) => {
                authed = sess.authenticated();
            }
            Err(e) => {
                // 记录错误，继续尝试其他认证方式
                let _ = e;
            }
        }

        // 2) 回退：尝试键盘交互认证
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
                    let mut responses = Vec::with_capacity(prompts.len());
                    for _ in prompts {
                        responses.push(self.pwd.clone());
                    }
                    responses
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
            return Err(anyhow::anyhow!(
                "无法创建SFTP会话，请检查SSH凭据或服务器认证方式"
            ));
        }

        let sftp = sess.sftp()?;
        let sftp_session = SftpSession { ssh: sess, sftp };

        let mut sessions = self.sessions.lock().await;
        // 复用空槽位
        if let Some((idx, slot)) = sessions.iter_mut().enumerate().find(|(_, s)| s.is_none()) {
            *slot = Some(sftp_session);
            return Ok(idx);
        }
        sessions.push(Some(sftp_session));
        Ok(sessions.len() - 1)
    }

    pub async fn get_session(&self, session_id: usize) -> anyhow::Result<SftpSessionGuard> {
        let sessions = self.sessions.lock().await;
        if let Some(Some(_)) = sessions.get(session_id) { /* ok */
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
    service: Arc<Mutex<Vec<Option<SftpSession>>>>,
    id: usize,
}

impl SftpSessionGuard {
    fn with<F, T>(&self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(&SftpSession) -> anyhow::Result<T>,
    {
        let sessions = futures::executor::block_on(self.service.lock());
        let sess = sessions
            .get(self.id)
            .and_then(|s| s.as_ref())
            .ok_or_else(|| anyhow::anyhow!("会话不存在"))?;
        f(sess)
    }

    pub fn list(&self, path: &str) -> anyhow::Result<Vec<FileEntry>> {
        self.with(|sess| {
            let mut out = Vec::new();
            let entries = sess.sftp.readdir(std::path::Path::new(path))?;
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
    }

    pub fn read_text(&self, path: &str) -> anyhow::Result<String> {
        self.with(|sess| {
            let mut file = sess.sftp.open(std::path::Path::new(path))?;
            let mut buf = Vec::new();
            use std::io::Read;
            file.read_to_end(&mut buf)?;
            let s = String::from_utf8(buf).unwrap_or_else(|_| String::from("(非文本文件)"));
            Ok(s)
        })
    }

    pub fn write_text(&self, path: &str, content: &str) -> anyhow::Result<()> {
        self.with(|sess| {
            let mut file = sess.sftp.create(std::path::Path::new(path))?;
            use std::io::Write;
            file.write_all(content.as_bytes())?;
            Ok(())
        })
    }

    pub fn delete(&self, path: &str) -> anyhow::Result<()> {
        self.with(|sess| {
            // 先尝试删除文件，失败则尝试删除目录
            let p = std::path::Path::new(path);
            if let Err(_) = sess.sftp.unlink(p) {
                sess.sftp.rmdir(p)?;
            }
            Ok(())
        })
    }

    pub fn rename(&self, from: &str, to_name: &str) -> anyhow::Result<()> {
        self.with(|sess| {
            // 目标名在同目录下
            let from_path = std::path::Path::new(from);
            let parent = from_path.parent().unwrap_or(std::path::Path::new("/"));
            let to_path = parent.join(to_name);
            sess.sftp.rename(from_path, &to_path, None)?;
            Ok(())
        })
    }

    pub fn upload_base64(
        &self,
        dest_dir: &str,
        filename: &str,
        base64: &str,
    ) -> anyhow::Result<()> {
        self.with(|sess| {
            use base64::prelude::*;
            let decoded = BASE64_STANDARD.decode(base64)?;
            let dest_path = if dest_dir.ends_with('/') {
                format!("{}{}", dest_dir, filename)
            } else {
                format!("{}/{}", dest_dir, filename)
            };
            let dest_path = std::path::Path::new(&dest_path);
            let mut file = sess.sftp.create(dest_path)?;
            use std::io::Write;
            file.write_all(&decoded)?;
            Ok(())
        })
    }

    pub fn download(&self, path: &str) -> anyhow::Result<Vec<u8>> {
        self.with(|sess| {
            let mut file = sess.sftp.open(std::path::Path::new(path))?;
            let mut buf = Vec::new();
            use std::io::Read;
            file.read_to_end(&mut buf)?;
            Ok(buf)
        })
    }

    pub fn mkdir(&self, path: &str) -> anyhow::Result<()> {
        self.with(|sess| {
            // mode 0o755 = 493 (rwxr-xr-x)
            sess.sftp.mkdir(std::path::Path::new(path), 0o755)?;
            Ok(())
        })
    }

    pub fn chmod(&self, path: &str, mode: i32) -> anyhow::Result<()> {
        self.with(|sess| {
            let mut stat = sess.sftp.stat(std::path::Path::new(path))?;
            stat.perm = Some(mode as u32);
            sess.sftp.setstat(std::path::Path::new(path), stat)?;
            Ok(())
        })
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
