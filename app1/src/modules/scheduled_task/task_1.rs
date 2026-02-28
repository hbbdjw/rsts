use tokio_cron_scheduler::{Job, JobScheduler, JobSchedulerError};

// 导入日志宏
use crate::log_debug;
// use crate::log_error;
use crate::log_info;
// use crate::log_warn;

/// 启动定时任务模块
///
/// 此函数演示了tokio_cron_scheduler库的多种定时任务用法，包括：
/// - 基本cron表达式任务
/// - 异步cron表达式任务
/// - 英文描述格式的任务
/// - 一次性任务
/// - 重复执行任务
/// - 任务生命周期通知处理
///
/// 返回值: 成功返回Ok(()), 失败返回JobSchedulerError
pub async fn start_scheduled_tasks() -> Result<(), JobSchedulerError> {
    // 创建一个新的任务调度器实例
    let mut sched = JobScheduler::new().await?;

    // 添加基本的cron表达式任务 - 每10秒执行一次
    // 6字段cron表达式格式: 秒 分 时 日 月 周
    sched
        .add(Job::new("1/10 * * * * *", |_uuid, _l| {
            // println!("基本任务: 每10秒执行一次");
        })?)
        .await?;

    // 添加异步cron任务 - 每7秒执行一次
    sched
        .add(Job::new_async("1/7 * * * * *", |uuid, mut l| {
            Box::pin(async move {
                // println!("异步任务: 每7秒执行一次");

                // 查询此任务的下一次执行时间
                let next_tick = l.next_tick_for_job(uuid).await;
                match next_tick {
                    Ok(Some(ts)) => {
                        log_debug!("7秒任务的下一次执行时间: {:?}", ts);
                    }
                    _ => {
                        log_debug!("无法获取7秒任务的下一次执行时间");
                    }
                }
            })
        })?)
        .await?;

    // 添加英文描述格式的异步任务 - 每4秒执行一次
    // 需要在Cargo.toml中启用`english`特性
    // sched
    //     .add(Job::new_async("every 4 seconds", |uuid, mut l| {
    //         Box::pin(async move {
    //             println!("英文描述任务: 每4秒执行一次");

    //             // 查询此任务的下一次执行时间
    //             let next_tick = l.next_tick_for_job(uuid).await;
    //             match next_tick {
    //                 Ok(Some(ts)) => println!("4秒任务的下一次执行时间: {:?}", ts),
    //                 _ => println!("无法获取4秒任务的下一次执行时间"),
    //             }
    //         })
    //     })?)
    //     .await?;

    // 添加一次性任务 - 18秒后执行一次
    // sched
    //     .add(Job::new_one_shot(Duration::from_secs(18), |_uuid, _l| {
    //         println!("一次性任务: 只执行一次");
    //     })?)
    //     .await?;

    // 创建一个重复执行的任务 - 每8秒执行一次
    // 使用mut关键字使任务可变，以便后续添加通知处理
    // let mut jj = Job::new_repeated(Duration::from_secs(8), |_uuid, _l| {
    //     println!("重复任务: 每8秒执行一次");
    // })?;

    // 为任务添加启动时的通知处理
    // jj.on_start_notification_add(
    //     &sched,
    //     Box::new(|job_id, notification_id, type_of_notification| {
    //         Box::pin(async move {
    //             println!(
    //                 "任务通知: 任务 {:?} 已启动, 通知 {:?} 已执行 (类型: {:?})",
    //                 job_id, notification_id, type_of_notification
    //             );
    //         })
    //     }),
    // )
    // .await?;

    // 为任务添加完成时的通知处理
    // jj.on_stop_notification_add(
    //     &sched,
    //     Box::new(|job_id, notification_id, type_of_notification| {
    //         Box::pin(async move {
    //             println!(
    //                 "任务通知: 任务 {:?} 已完成, 通知 {:?} 已执行 (类型: {:?})",
    //                 job_id, notification_id, type_of_notification
    //             );
    //         })
    //     }),
    // )
    // .await?;

    // 为任务添加移除时的通知处理
    // jj.on_removed_notification_add(
    //     &sched,
    //     Box::new(|job_id, notification_id, type_of_notification| {
    //         Box::pin(async move {
    //             println!(
    //                 "任务通知: 任务 {:?} 已移除, 通知 {:?} 已执行 (类型: {:?})",
    //                 job_id, notification_id, type_of_notification
    //             );
    //         })
    //     }),
    // )
    // .await?;

    // 将配置好的重复任务添加到调度器
    // 注意: 使用信号处理功能需要在Cargo.toml中启用`signal`特性
    // sched.add(jj).await?;

    // 设置调度器关闭时的处理函数
    sched.set_shutdown_handler(Box::new(|| {
        Box::pin(async move {
            log_info!("调度器已成功关闭");
        })
    }));

    // 启动调度器，开始执行所有添加的任务
    sched.start().await?;

    // 创建一个永远不完成的future，让定时任务持续运行
    futures::future::pending::<()>().await;

    // 这个代码永远不会执行到这里
    if let Err(e) = sched.shutdown().await {
        log_info!("调度器关闭时出错: {:?}", e);
    }
    Ok(())
}
