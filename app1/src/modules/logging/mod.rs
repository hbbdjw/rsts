use env_logger::{Builder, Target};
// use log::{debug, error, info, warn};
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

// 创建一个全局的日志文件句柄
static LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// 初始化日志系统
///
/// # Arguments
/// * `log_file_path` - 日志文件路径
/// * `log_level` - 日志级别 (debug, info, warn, error)
pub fn init_logger(log_file_path: &str, log_level: &str) {
    // 打开日志文件
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(log_file_path)
        .expect("Failed to open log file");

    // 保存文件句柄
    *LOG_FILE.lock().unwrap() = Some(file);

    // 设置环境变量 - 配置特定库的日志级别以过滤掉不需要的日志
    // 格式: RUST_LOG=全局级别,库名=库级别
    // 关闭或降低tokio和actix相关库的日志级别
    unsafe {
        std::env::set_var(
            "RUST_LOG",
            format!(
                "{},tokio=error,actix_server=error,actix_web=info,actix_rt=error",
                log_level
            ),
        );
    }

    // 配置env_logger
    Builder::from_default_env().target(Target::Stdout).init();
}

/// 写入日志到文件
///
/// # Arguments
/// * `level` - 日志级别
/// * `message` - 日志消息
pub fn log_to_file(level: &str, message: &str) {
    if let Some(ref mut file) = *LOG_FILE.lock().unwrap() {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let log_entry = format!("[{}] [{}] {}\n", timestamp, level, message);
        if let Err(e) = writeln!(file, "{}", log_entry) {
            eprintln!("Failed to write to log file: {}", e);
        }
        // 刷新文件以确保日志立即写入
        file.flush().unwrap();
    }
}

/// 刷新并关闭日志文件
/// 用于程序优雅退出时确保所有日志都被写入
pub fn flush_and_close_log() {
    if let Some(ref mut file) = *LOG_FILE.lock().unwrap() {
        let _ = file.flush();
    }
}

// 导出日志宏
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {{
        let message = format!($($arg)*);
        log::debug!("{}", message);
        $crate::modules::logging::log_to_file("DEBUG", &message);
    }};
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {{
        let message = format!($($arg)*);
        log::info!("{}", message);
        $crate::modules::logging::log_to_file("INFO", &message);
    }};
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {{
        let message = format!($($arg)*);
        log::warn!("{}", message);
        $crate::modules::logging::log_to_file("WARN", &message);
    }};
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {{
        let message = format!($($arg)*);
        log::error!("{}", message);
        $crate::modules::logging::log_to_file("ERROR", &message);
    }};
}
