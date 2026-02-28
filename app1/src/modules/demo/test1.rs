use log::info as log_info;
use std::thread;

pub fn run() {
    log_info!("=== test1 start ===");
    let count = match thread::available_parallelism() {
        Ok(v) => v.get(),
        Err(_) => 1, // 若获取失败，默认按单核处理
    };
    println!("Available CPUs: {}", count); // 输出系统逻辑CPU核心数

    log_info!("=== test1 end ===");
}
