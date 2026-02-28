use actix_web::{HttpResponse, Responder, web};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::models::{
    ChmodPayload, CreateSessionPayload, DeletePayload, MkdirPayload, PathQuery, RenamePayload,
    UploadPayload, WriteFilePayload,
};
use crate::modules::sftp::service::{SftpCredentials, SftpService};

// 数据模型已迁移至 models.rs

pub async fn create_session(
    service: web::Data<Arc<Mutex<SftpService>>>,
    payload: web::Json<CreateSessionPayload>,
) -> impl Responder {
    let creds = SftpCredentials {
        hostname: payload.hostname.clone(),
        port: payload.port,
        username: payload.username.clone(),
        password: payload.password.clone(),
    };
    let service = service.get_ref().clone();
    match service.lock().await.create_session(creds).await {
        Ok(id) => HttpResponse::Ok().json(serde_json::json!({
            "code": "0000",
            "msg": "success",
            "data": {"session_id": id}
        })),
        Err(e) => HttpResponse::Ok().json(serde_json::json!({
            "code": "5000",
            "msg": e.to_string(),
            "data": null
        })),
    }
}

pub async fn list_dir(
    service: web::Data<Arc<Mutex<SftpService>>>,
    q: web::Query<PathQuery>,
) -> impl Responder {
    let path = q.path.clone().unwrap_or("/".to_string());
    let service = service.get_ref().clone();
    let guard = match service.lock().await.get_session(q.session_id).await {
        Ok(g) => g,
        Err(e) => {
            return HttpResponse::Ok().json(serde_json::json!({
                "code": "5000",
                "msg": e.to_string(),
                "data": null
            }));
        }
    };
    match guard.list(&path) {
        Ok(entries) => HttpResponse::Ok().json(serde_json::json!({
            "code": "0000",
            "msg": "success",
            "data": entries
        })),
        Err(e) => HttpResponse::Ok().json(serde_json::json!({
            "code": "5000",
            "msg": e.to_string(),
            "data": null
        })),
    }
}

pub async fn read_file(
    service: web::Data<Arc<Mutex<SftpService>>>,
    q: web::Query<PathQuery>,
) -> impl Responder {
    let path = q.path.clone().unwrap_or("/".to_string());
    let service = service.get_ref().clone();
    let guard = match service.lock().await.get_session(q.session_id).await {
        Ok(g) => g,
        Err(e) => {
            return HttpResponse::Ok().json(serde_json::json!({
                "code": "5000",
                "msg": e.to_string(),
                "data": null
            }));
        }
    };
    match guard.read_text(&path) {
        Ok(text) => HttpResponse::Ok().json(serde_json::json!({
            "code": "0000",
            "msg": "success",
            "data": {"content": text}
        })),
        Err(e) => HttpResponse::Ok().json(serde_json::json!({
            "code": "5000",
            "msg": e.to_string(),
            "data": null
        })),
    }
}

pub async fn write_file(
    service: web::Data<Arc<Mutex<SftpService>>>,
    payload: web::Json<WriteFilePayload>,
) -> impl Responder {
    let service = service.get_ref().clone();
    let guard = match service.lock().await.get_session(payload.session_id).await {
        Ok(g) => g,
        Err(e) => {
            return HttpResponse::Ok().json(serde_json::json!({
                "code": "5000",
                "msg": e.to_string(),
                "data": null
            }));
        }
    };
    match guard.write_text(&payload.path, &payload.content) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "code": "0000",
            "msg": "success",
            "data": null
        })),
        Err(e) => HttpResponse::Ok().json(serde_json::json!({
            "code": "5000",
            "msg": e.to_string(),
            "data": null
        })),
    }
}

pub async fn delete_file(
    service: web::Data<Arc<Mutex<SftpService>>>,
    payload: web::Json<DeletePayload>,
) -> impl Responder {
    let service = service.get_ref().clone();
    let guard = match service.lock().await.get_session(payload.session_id).await {
        Ok(g) => g,
        Err(e) => {
            return HttpResponse::Ok().json(serde_json::json!({
                "code": "5000",
                "msg": e.to_string(),
                "data": null
            }));
        }
    };
    match guard.delete(&payload.path) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "code": "0000",
            "msg": "success",
            "data": "OK"
        })),
        Err(e) => HttpResponse::Ok().json(serde_json::json!({
            "code": "5000",
            "msg": e.to_string(),
            "data": null
        })),
    }
}

pub async fn rename_file(
    service: web::Data<Arc<Mutex<SftpService>>>,
    payload: web::Json<RenamePayload>,
) -> impl Responder {
    let service = service.get_ref().clone();
    let guard = match service.lock().await.get_session(payload.session_id).await {
        Ok(g) => g,
        Err(e) => {
            return HttpResponse::Ok().json(serde_json::json!({
                "code": "5000",
                "msg": e.to_string(),
                "data": null
            }));
        }
    };
    match guard.rename(&payload.path, &payload.new_name) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "code": "0000",
            "msg": "success",
            "data": "OK"
        })),
        Err(e) => HttpResponse::Ok().json(serde_json::json!({
            "code": "5000",
            "msg": e.to_string(),
            "data": null
        })),
    }
}

pub async fn upload_file(
    service: web::Data<Arc<Mutex<SftpService>>>,
    payload: web::Json<UploadPayload>,
) -> impl Responder {
    let service = service.get_ref().clone();
    let guard = match service.lock().await.get_session(payload.session_id).await {
        Ok(g) => g,
        Err(e) => {
            return HttpResponse::Ok().json(serde_json::json!({
                "code": "5000",
                "msg": e.to_string(),
                "data": null
            }));
        }
    };

    let path = payload.path.clone();
    let filename = payload.filename.clone();
    let content_base64 = payload.content_base64.clone();

    // Move blocking operation to thread pool
    let result = web::block(move || guard.upload_base64(&path, &filename, &content_base64)).await;

    match result {
        Ok(Ok(())) => HttpResponse::Ok().json(serde_json::json!({
            "code": "0000",
            "msg": "success",
            "data": "OK"
        })),
        Ok(Err(e)) => HttpResponse::Ok().json(serde_json::json!({
            "code": "5000",
            "msg": e.to_string(),
            "data": null
        })),
        Err(e) => HttpResponse::Ok().json(serde_json::json!({
            "code": "5000",
            "msg": format!("Blocking error: {}", e),
            "data": null
        })),
    }
}

pub async fn download_file(
    service: web::Data<Arc<Mutex<SftpService>>>,
    q: web::Query<PathQuery>,
) -> impl Responder {
    let path = q.path.clone().unwrap_or("/".to_string());
    let service = service.get_ref().clone();
    let guard = match service.lock().await.get_session(q.session_id).await {
        Ok(g) => g,
        Err(e) => return HttpResponse::BadRequest().body(e.to_string()),
    };

    // Move blocking operation to thread pool
    let path_clone = path.clone();
    let result = web::block(move || guard.download(&path_clone)).await;

    match result {
        Ok(Ok(bytes)) => {
            let fname = std::path::Path::new(&path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("download");
            HttpResponse::Ok()
                .append_header(("Content-Type", "application/octet-stream"))
                .append_header((
                    "Content-Disposition",
                    format!("attachment; filename=\"{}\"", fname),
                ))
                .body(bytes)
        }
        Ok(Err(e)) => HttpResponse::NotFound().body(e.to_string()),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

pub async fn create_dir(
    service: web::Data<Arc<Mutex<SftpService>>>,
    payload: web::Json<MkdirPayload>,
) -> impl Responder {
    let service = service.get_ref().clone();
    let guard = match service.lock().await.get_session(payload.session_id).await {
        Ok(g) => g,
        Err(e) => {
            return HttpResponse::Ok().json(serde_json::json!({
                "code": "5000",
                "msg": e.to_string(),
                "data": null
            }));
        }
    };
    match guard.mkdir(&payload.path) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "code": "0000",
            "msg": "success",
            "data": "OK"
        })),
        Err(e) => HttpResponse::Ok().json(serde_json::json!({
            "code": "5000",
            "msg": e.to_string(),
            "data": null
        })),
    }
}

pub async fn set_permissions(
    service: web::Data<Arc<Mutex<SftpService>>>,
    payload: web::Json<ChmodPayload>,
) -> impl Responder {
    let service = service.get_ref().clone();
    let guard = match service.lock().await.get_session(payload.session_id).await {
        Ok(g) => g,
        Err(e) => {
            return HttpResponse::Ok().json(serde_json::json!({
                "code": "5000",
                "msg": e.to_string(),
                "data": null
            }));
        }
    };
    match guard.chmod(&payload.path, payload.mode) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "code": "0000",
            "msg": "success",
            "data": "OK"
        })),
        Err(e) => HttpResponse::Ok().json(serde_json::json!({
            "code": "5000",
            "msg": e.to_string(),
            "data": null
        })),
    }
}
