use actix_web::{HttpResponse, Responder, web};
use std::fs;
use std::path::PathBuf;

use super::models::UploadChatMediaPayload;
// 数据模型已迁移至 models.rs

fn ensure_upload_dir() -> std::io::Result<PathBuf> {
    let base = PathBuf::from("static/uploads/chat");
    if !base.exists() {
        fs::create_dir_all(&base)?;
    }
    Ok(base)
}

pub async fn upload_media(payload: web::Json<UploadChatMediaPayload>) -> impl Responder {
    // 基本校验
    let media_type = payload.media_type.as_str();
    if media_type != "image" && media_type != "video" && media_type != "audio" {
        return HttpResponse::BadRequest().body("invalid media_type");
    }

    let upload_dir = match ensure_upload_dir() {
        Ok(p) => p,
        Err(e) => {
            return HttpResponse::InternalServerError().body(format!("create dir error: {}", e));
        }
    };

    // 解析base64（允许带data URL前缀）
    let b64 = if let Some(idx) = payload.content_base64.find(",") {
        payload.content_base64[idx + 1..].to_string()
    } else {
        payload.content_base64.clone()
    };

    let bytes = match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &b64) {
        Ok(b) => b,
        Err(e) => return HttpResponse::BadRequest().body(format!("base64 decode error: {}", e)),
    };

    // 生成安全文件名
    let fname = {
        let ts = chrono::Utc::now().timestamp_millis();
        let sanitized = payload
            .filename
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
            .collect::<String>();
        if sanitized.is_empty() {
            format!("{}_upload.bin", ts)
        } else {
            format!("{}_{}", ts, sanitized)
        }
    };

    let mut path = upload_dir.clone();
    path.push(&fname);

    if let Err(e) = fs::write(&path, &bytes) {
        return HttpResponse::InternalServerError().body(format!("write file error: {}", e));
    }

    // 构造可访问的URL（静态文件根为/static 映射到 /）
    let url = format!("/uploads/chat/{}", fname);
    HttpResponse::Ok().json(serde_json::json!({
        "url": url,
        "media_type": media_type,
    }))
}
