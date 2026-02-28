use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use toml;

// 配置文件结构定义
// WebSocket配置
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WebSocketConfig {
    pub enabled: bool,
    pub path: String,
    pub heartbeat_interval: u64,
    pub max_frame_size: usize,
    pub max_connections: u32,
}

// 日志配置
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LogConfig {
    pub file_path: String,
    pub level: String,
}

// TCP代理配置
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TcpProxyConfig {
    pub enable: bool,
    pub port: u16,
    pub rules: Vec<TcpProxyRule>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TcpProxyRule {
    pub local_port: u16,
    pub remote_addresses: Vec<String>,
    pub pattern: Option<String>,
}

// 代理配置
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProxyConfig {
    pub enable: bool,
    pub rules: Vec<ProxyRule>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProxyRule {
    // 匹配路径的前缀
    pub path_prefix: String,
    // 转发目标的基础URL
    pub target_url: String,
    // 可选的匹配正则表达式
    pub pattern: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub static_files: StaticFilesConfig,
    pub scheduled_task: ScheduledTaskConfig,
    pub proxy: ProxyConfig,
    pub websocket: WebSocketConfig,
    pub log: LogConfig,
    pub tcp_proxy: TcpProxyConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub host: String,
    pub database_path: String,
    pub enable: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StaticFilesConfig {
    pub root_dir: String,
    pub allowed_extensions: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ScheduledTaskConfig {
    pub enable: bool,
    pub task_1: TaskConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TaskConfig {
    pub cron_expression: String,
    pub description: String,
}

impl Config {
    // 从配置文件读取配置
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // 检查配置文件是否存在
        let config_path = Path::new(path);
        if !config_path.exists() {
            // 如果配置文件不存在，创建默认配置文件
            let default_config = Config::default();
            default_config.save_to_file(path)?;
            return Ok(default_config);
        }

        // 读取配置文件内容
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        // 解析配置文件
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    // 保存配置到文件
    pub fn save_to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        // 确保配置文件所在目录存在
        if let Some(parent) = Path::new(path).parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // 将配置序列化为TOML格式
        let toml_str = toml::to_string_pretty(&self)?;

        // 写入文件
        std::fs::write(path, toml_str)?;
        Ok(())
    }

    // 获取默认配置
    pub fn default() -> Self {
        Self {
            server: ServerConfig {
                port: 8000,
                host: "127.0.0.1".to_string(),
                database_path: "./db/rsts.db".to_string(),
                enable: true,
            },
            static_files: StaticFilesConfig {
                root_dir: "./static".to_string(),
                allowed_extensions: "html,css,js,png".to_string(),
            },
            scheduled_task: ScheduledTaskConfig {
                enable: true,
                task_1: TaskConfig {
                    cron_expression: "* * * * *".to_string(),
                    description: "每分钟执行一次的示例任务".to_string(),
                },
            },
            proxy: ProxyConfig {
                enable: true,
                rules: vec![ProxyRule {
                    path_prefix: "/api/remote".to_string(),
                    target_url: "http://127.0.0.1:9000".to_string(),
                    pattern: None,
                }],
            },
            websocket: WebSocketConfig {
                enabled: true,
                path: "/ws".to_string(),
                heartbeat_interval: 5,
                max_frame_size: 65536,
                max_connections: 100,
            },
            log: LogConfig {
                file_path: "logs.log".to_string(),
                level: "info".to_string(),
            },
            tcp_proxy: TcpProxyConfig {
                enable: false,
                port: 8080,
                rules: vec![TcpProxyRule {
                    local_port: 8080,
                    remote_addresses: vec!["127.0.0.1:9001".to_string()],
                    pattern: None,
                }],
            },
        }
    }
}

// 方便使用的配置单例
pub fn get_config() -> Result<Config, Box<dyn std::error::Error>> {
    static mut CONFIG: Option<Config> = None;

    unsafe {
        let config_ptr: *mut Option<Config> = &raw mut CONFIG;
        if (*config_ptr).is_none() {
            let path = if Path::new("config.toml").exists() {
                "config.toml"
            } else if Path::new("app1/config.toml").exists() {
                "app1/config.toml"
            } else {
                "config.toml"
            };
            *config_ptr = Some(Config::from_file(path)?);
        }
        Ok((*config_ptr).clone().unwrap())
    }
}
