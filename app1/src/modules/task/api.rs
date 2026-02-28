use actix_web::{HttpResponse, Responder, web};
use serde::Deserialize;

use crate::modules::task::service::{TaskConfig, TaskManager};

#[derive(Deserialize, Debug)]
pub struct StartTaskPayload {
    pub threads: Option<usize>,
    pub pause_ms: Option<u64>,
    pub duration_seconds: Option<u64>, // None 表示无限制
}

pub async fn start_task(
    task_mgr: web::Data<TaskManager>,
    payload: web::Json<StartTaskPayload>,
) -> impl Responder {
    let cfg = TaskConfig {
        threads: payload
            .threads
            .unwrap_or_else(|| num_cpus::get_physical().max(1)),
        pause_ms: payload.pause_ms.unwrap_or(500),
        duration_seconds: payload.duration_seconds,
    };
    task_mgr.start(cfg).await;
    HttpResponse::Ok().json(serde_json::json!({"status":"started"}))
}

pub async fn stop_task(task_mgr: web::Data<TaskManager>) -> impl Responder {
    task_mgr.stop().await;
    HttpResponse::Ok().json(serde_json::json!({"status":"stopped"}))
}

pub async fn task_status(task_mgr: web::Data<TaskManager>) -> impl Responder {
    let status = task_mgr.status().await;
    HttpResponse::Ok().json(status)
}
