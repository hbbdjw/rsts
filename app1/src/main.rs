#![cfg_attr(feature = "simd", feature(portable_simd))]
// 导入所需模块
use futures::channel::oneshot;
use tokio::runtime::Builder;
use tokio::signal;
use tokio::task::LocalSet;

// 导入modules目录
mod modules;

// 主函数 - 管理程序生命周期
fn main() {
    // 获取配置并初始化日志系统
    let config: modules::config::config::Config =
        modules::config::config::get_config().expect("Failed to load configuration");
    modules::logging::init_logger(&config.log.file_path, &config.log.level);
    log_info!("=== rsts start ===");

    // 创建一个支持LocalSet的Tokio运行时
    let rt = Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build Tokio runtime");

    // 创建LocalSet
    let local_set = LocalSet::new();

    // 在LocalSet中运行所有模块
    local_set.block_on(&rt, async move {
        // 创建一个用于通知优雅关闭的信号
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();

        // 启动信号处理任务
        tokio::task::spawn_local(async move {
            // 等待SIGINT信号（Ctrl+C）
            if let Err(err) = signal::ctrl_c().await {
                log_error!("无法监听终止信号: {}", err);
                return;
            }

            log_info!("收到终止信号，开始关闭...");
            // 发送关闭信号给主任务
            let _ = shutdown_sender.send(());
        });

        // 启动所有启用的模块
        let mut tasks = Vec::new();

        // 启用web模块
        if config.server.enable {
            log_info!("=== Web服务器 ===");
            tasks.push(tokio::task::spawn_local(async {
                if let Err(err) = modules::web::main_web::run().await {
                    log_error!("Web服务器错误: {}", err);
                }
            }));
        }

        // 启用定时任务模块
        if config.scheduled_task.enable {
            log_info!("=== 定时任务模块 ===");
            tasks.push(tokio::task::spawn_local(async {
                if let Err(err) = modules::scheduled_task::task_1::start_scheduled_tasks().await {
                    log_error!("定时任务模块错误: {}", err);
                }
            }));
        }

        // 启用代理模块
        if config.proxy.enable {
            log_info!("=== HTTP代理中间件已集成到Web服务器 ===");
            log_info!("=== TCP代理模块 ===");
            tasks.push(tokio::task::spawn_local(async {
                if let Err(err) = modules::proxy::proxy_tcp::start_tcp_proxy().await {
                    log_error!("TCP代理模块错误: {}", err);
                }
            }));
        }

        // 等待关闭信号或所有任务完成
        tokio::select! {
            // 等待关闭信号
            _ = shutdown_receiver => {
                log_info!("正在关闭所有模块...");
                // 不需要取消任务，因为我们使用了select，当收到关闭信号时，
                // 我们会继续执行并正常退出
            },
            // 等待所有任务完成
            _ = async {
                for task in tasks {
                    let _ = task.await;
                }
            } => {
                // 所有任务已完成
                log_info!("所有模块已完成");
            }
        }

        // 退出前的清理工作
        log_info!("执行清理工作...");

        // 刷新并关闭日志文件
        modules::logging::flush_and_close_log();

        log_info!("=== rsts 退出 ===");
    });
}
