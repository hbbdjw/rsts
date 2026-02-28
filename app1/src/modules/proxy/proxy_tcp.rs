use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// 导入配置模块的TcpProxyConfig类型
use crate::modules::config::config::TcpProxyConfig;
use crate::modules::config::config::TcpProxyRule;
use crate::modules::config::config::get_config;
// TCP代理服务器
pub struct TcpProxyServer {
    config: Arc<TcpProxyConfig>,
    // 每个规则的轮询计数器，用于负载均衡
    counters: Arc<Mutex<Vec<usize>>>,
}

impl TcpProxyServer {
    pub fn new(config: TcpProxyConfig) -> Self {
        // 初始化每个规则的计数器为0
        let counters = vec![0; config.rules.len()];
        Self {
            config: Arc::new(config),
            counters: Arc::new(Mutex::new(counters)),
        }
    }

    // 启动TCP代理服务器
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.config.enable {
            // TCP代理已禁用
            return Ok(());
        }

        let bind_addr = format!("127.0.0.1:{}", self.config.port);
        let _listener = TcpListener::bind(bind_addr).await?;
        // TCP代理服务器启动在配置端口

        // 为每个规则启动单独的监听器
        for (index, rule) in self.config.rules.iter().enumerate() {
            let rule_clone = rule.clone();
            let counters_clone = self.counters.clone();
            tokio::spawn(async move {
                if let Err(_e) = Self::start_rule_listener(rule_clone, index, counters_clone).await
                {
                    // TCP代理规则监听失败
                }
            });
        }

        // 主循环可以保持运行
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    }

    // 为单个规则启动监听器
    async fn start_rule_listener(
        rule: TcpProxyRule,
        rule_index: usize,
        counters: Arc<Mutex<Vec<usize>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bind_addr = format!("127.0.0.1:{}", rule.local_port);
        let listener = TcpListener::bind(bind_addr).await?;
        // TCP代理规则监听启动

        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    // 接收到TCP连接
                    let rule_clone = rule.clone();
                    let counters_clone = counters.clone();
                    tokio::spawn(async move {
                        if let Err(_e) =
                            Self::handle_connection(stream, rule_clone, rule_index, counters_clone)
                                .await
                        {
                            // 处理TCP连接失败
                        }
                    });
                }
                Err(_e) => {
                    // 接受TCP连接失败
                }
            }
        }
    }

    // 处理单个TCP连接
    async fn handle_connection(
        mut stream: TcpStream,
        rule: TcpProxyRule,
        rule_index: usize,
        counters: Arc<Mutex<Vec<usize>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 实现轮询负载均衡，选择目标地址
        let target_address = {
            let mut counters = counters.lock().unwrap();
            let counter = &mut counters[rule_index];
            let index = *counter % rule.remote_addresses.len();
            *counter += 1;
            rule.remote_addresses[index].clone()
        };

        // 连接到选定的目标服务器
        let mut target_stream = TcpStream::connect(&target_address).await?;
        // 成功连接到目标服务器

        // 双向复制数据
        let (mut client_reader, mut client_writer) = stream.split();
        let (mut target_reader, mut target_writer) = target_stream.split();

        // 复制客户端到目标
        let client_to_target = async {
            let mut buf = [0; 4096];
            loop {
                match client_reader.read(&mut buf).await {
                    Ok(0) => break, // 连接关闭
                    Ok(n) => {
                        if let Err(_e) = target_writer.write_all(&buf[..n]).await {
                            // 写入目标服务器失败
                            eprintln!("Error writing to target: {:?}", _e);
                            break;
                        }
                    }
                    Err(_e) => {
                        // 从客户端读取数据失败
                        eprintln!("Error reading from client: {:?}", _e);
                        break;
                    }
                }
            }
        };

        let target_to_client = async {
            let mut buf = [0; 4096];
            loop {
                match target_reader.read(&mut buf).await {
                    Ok(0) => break, // 连接关闭
                    Ok(n) => {
                        if let Err(_e) = client_writer.write_all(&buf[..n]).await {
                            // 写入客户端失败
                            eprintln!("Error writing to client: {:?}", _e);
                            break;
                        }
                    }
                    Err(_e) => {
                        // 从目标服务器读取数据失败
                        eprintln!("Error reading from target: {:?}", _e);
                        break;
                    }
                }
            }
        };

        // 等待任一方向完成
        tokio::select! {
            _ = client_to_target => {},
            _ = target_to_client => {},
        }

        // TCP连接已关闭
        Ok(())
    }

    // 检查连接是否应该被转发
    pub fn should_forward(&self, local_port: u16) -> Option<TcpProxyRule> {
        for rule in &self.config.rules {
            if rule.local_port == local_port {
                return Some(rule.clone());
            }
        }
        None
    }
}

// 方便使用的TCP代理启动函数
pub async fn start_tcp_proxy() -> Result<(), Box<dyn std::error::Error>> {
    // 从配置文件加载TCP代理配置
    let config = get_config().expect("Failed to load configuration");

    if !config.tcp_proxy.enable {
        // TCP代理已禁用
        return Ok(());
    }

    // 创建并启动TCP代理服务器
    let tcp_proxy = TcpProxyServer::new(config.tcp_proxy);
    tcp_proxy.start().await
}
