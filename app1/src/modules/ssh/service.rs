use std::sync::Arc;
use tokio::sync::Mutex;

use crate::modules::ssh::ssh_client::{SshClient, SshCredentials, SshError, SshResult};
use crate::{log_error, log_info};

// SSH服务结构体
pub struct SshService {
    // 存储SSH客户端连接
    clients: Arc<Mutex<Vec<Arc<Mutex<SshClient>>>>>,
}

impl SshService {
    // 创建新的SSH服务
    pub fn new() -> Self {
        Self {
            clients: Arc::new(Mutex::new(Vec::new())),
        }
    }

    // 启动SSH服务
    pub async fn start(&self) -> SshResult<()> {
        log_info!("SSH服务已启动");

        // SSH服务主要是提供API供其他模块调用，不需要长时间运行的任务
        // 这里可以添加定期清理空闲连接的任务

        Ok(())
    }

    // 创建新的SSH连接
    pub async fn create_connection(&self, credentials: SshCredentials) -> SshResult<usize> {
        let mut client = SshClient::new();
        client.connect(credentials.clone()).await?;

        let mut clients = self.clients.lock().await;
        let client_id = clients.len();
        clients.push(Arc::new(Mutex::new(client)));

        log_info!("创建新的SSH连接，连接ID: {}", client_id);

        Ok(client_id)
    }

    // 通过连接ID获取SSH客户端
    pub async fn get_client(&self, client_id: usize) -> SshResult<Arc<Mutex<SshClient>>> {
        let clients = self.clients.lock().await;

        if client_id >= clients.len() {
            log_error!("无效的SSH连接ID: {}", client_id);
            return Err(SshError::ConnectionError(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "无效的连接ID",
            )));
        }

        Ok(clients[client_id].clone())
    }

    // 关闭指定的SSH连接
    pub async fn close_connection(&self, client_id: usize) -> SshResult<()> {
        let mut clients = self.clients.lock().await;

        if client_id >= clients.len() {
            log_error!("无效的SSH连接ID: {}", client_id);
            return Err(SshError::ConnectionError(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "无效的连接ID",
            )));
        }

        // 移除并断开连接
        let client = clients.remove(client_id);
        if let Err(err) = client.lock().await.disconnect().await {
            log_error!("关闭SSH连接失败 (ID: {}): {:?}", client_id, err);
        }

        log_info!("SSH连接已关闭，连接ID: {}", client_id);

        Ok(())
    }

    // 关闭所有SSH连接
    #[allow(dead_code)]
    pub async fn close_all_connections(&self) -> SshResult<()> {
        let mut clients = self.clients.lock().await;
        let client_count = clients.len();

        // 断开所有连接
        for client in clients.iter_mut() {
            let _ = client.lock().await.disconnect().await;
        }

        // 清空客户端列表
        clients.clear();

        log_info!("已关闭所有SSH连接，共关闭 {} 个连接", client_count);

        Ok(())
    }

    // 获取当前活动连接数量
    #[allow(dead_code)]
    pub async fn get_active_connections_count(&self) -> usize {
        self.clients.lock().await.len()
    }
}

// 方便使用的SSH服务启动函数
#[allow(dead_code)]
pub async fn start_ssh_service() -> SshResult<()> {
    let ssh_service = SshService::new();
    ssh_service.start().await?;

    // 这里可以将ssh_service存储在全局状态中，供其他模块使用

    Ok(())
}
