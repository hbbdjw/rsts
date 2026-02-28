use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use log::{info, warn};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::sleep;

#[derive(Clone, Debug)]
pub struct TaskConfig {
    pub threads: usize,
    pub pause_ms: u64,
    pub duration_seconds: Option<u64>, // None 表示无限制
}

#[derive(Debug)]
pub struct TaskState {
    pub running: bool,
    pub start_time: Option<Instant>,
    pub config: Option<TaskConfig>,
    pub handles: Vec<JoinHandle<()>>, // 仅用于观察与等待
    pub stop_flag: Arc<std::sync::atomic::AtomicBool>,
}

impl TaskState {
    pub fn new() -> Self {
        Self {
            running: false,
            start_time: None,
            config: None,
            handles: Vec::new(),
            stop_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

#[derive(Clone)]
pub struct TaskManager {
    pub inner: Arc<Mutex<TaskState>>, // 用于跨请求共享并发状态
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(TaskState::new())),
        }
    }

    pub async fn start(&self, config: TaskConfig) {
        let mut state = self.inner.lock().await;
        // 若已运行，先停止
        if state.running {
            warn!("任务已在运行，先停止再重启");
            drop(state); // 解锁
            self.stop().await;
            state = self.inner.lock().await;
        }

        info!(
            "启动任务: threads={}, pause_ms={}, duration={:?}",
            config.threads, config.pause_ms, config.duration_seconds
        );

        state
            .stop_flag
            .store(false, std::sync::atomic::Ordering::SeqCst);
        state.running = true;
        state.start_time = Some(Instant::now());
        state.config = Some(config.clone());
        state.handles.clear();

        let stop_flag = state.stop_flag.clone();
        let threads = config.threads.max(1);
        let pause = Duration::from_millis(config.pause_ms);
        let deadline = config
            .duration_seconds
            .map(|s| Instant::now() + Duration::from_secs(s));

        for idx in 0..threads {
            let worker_flag = stop_flag.clone();
            let worker_deadline = deadline;
            let worker_pause = pause;
            let handle = tokio::spawn(async move {
                info!("[task-worker-{}] 启动", idx);
                loop {
                    // 检查停止标志
                    if worker_flag.load(std::sync::atomic::Ordering::SeqCst) {
                        info!("[task-worker-{}] 收到停止信号，退出", idx);
                        break;
                    }
                    // 检查总时长截止时间
                    if let Some(dl) = worker_deadline {
                        if Instant::now() >= dl {
                            info!("[task-worker-{}] 达到运行时长截止，退出", idx);
                            break;
                        }
                    }

                    // 执行任务：目前仅打印日志，可扩展为实际工作
                    info!(
                        "[task-worker-{}] 执行一次任务 @ {:?}",
                        idx,
                        SystemTime::now()
                    );

                    // 控制频率/间隔
                    sleep(worker_pause).await;
                }
                info!("[task-worker-{}] 已退出", idx);
            });
            state.handles.push(handle);
        }
    }

    pub async fn stop(&self) {
        let mut state = self.inner.lock().await;
        if !state.running {
            warn!("停止任务: 当前没有运行的任务");
            return;
        }
        info!("发送停止信号给所有任务线程");
        state
            .stop_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let handles = std::mem::take(&mut state.handles);
        state.running = false;
        drop(state); // 解锁再等待，避免死锁

        // 等待所有线程结束
        for h in handles {
            let _ = h.await;
        }
        info!("所有任务线程已停止");
    }

    pub async fn status(&self) -> TaskStatusDto {
        let state = self.inner.lock().await;
        let (threads, pause_ms, duration_seconds) = match &state.config {
            Some(c) => (c.threads, c.pause_ms, c.duration_seconds),
            None => (0, 0, None),
        };
        TaskStatusDto {
            running: state.running,
            threads,
            pause_ms,
            duration_seconds,
            active_workers: state.handles.len(),
            elapsed_seconds: state.start_time.map(|s| s.elapsed().as_secs()).unwrap_or(0),
        }
    }
}

#[derive(serde::Serialize)]
pub struct TaskStatusDto {
    pub running: bool,
    pub threads: usize,
    pub pause_ms: u64,
    pub duration_seconds: Option<u64>,
    pub active_workers: usize,
    pub elapsed_seconds: u64,
}
