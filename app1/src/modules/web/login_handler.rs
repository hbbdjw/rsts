use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, web};
use std::sync::Arc;

use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use super::models::{LoginRequest, Response};
use crate::modules::web::auth_utils::{
    ACCESS_TOKEN_EXPIRE_MINUTES, CODE_INVALID_CREDENTIALS, CODE_REFRESH_TOKEN_INVALID,
    CODE_SUCCESS, CODE_TOKEN_INVALID, CODE_USER_NOT_FOUND, Claims, JWT_SECRET,
    REFRESH_TOKEN_EXPIRE_DAYS,
};
use crate::modules::web::database::Database;

#[derive(Debug, Serialize)]
struct LoginToken {
    token: String,
    #[serde(rename = "refreshToken")]
    refresh_token: String,
}

#[derive(Debug, Serialize)]
struct FrontendUserInfo {
    #[serde(rename = "userId")]
    user_id: String,
    #[serde(rename = "userName")]
    user_name: String,
    roles: Vec<String>,
    buttons: Vec<String>,
}

fn build_user_info(id: i32, username: &str) -> FrontendUserInfo {
    let mut roles = vec!["R_USER".to_string()];
    if username.eq_ignore_ascii_case("admin") || username.eq_ignore_ascii_case("super") {
        roles.push("R_SUPER".to_string());
    }

    FrontendUserInfo {
        user_id: id.to_string(),
        user_name: username.to_string(),
        roles,
        buttons: vec![],
    }
}

fn generate_tokens(user_id: i32, username: &str) -> Result<LoginToken, HttpResponse> {
    let now = Utc::now();
    let access_exp = (now + Duration::minutes(ACCESS_TOKEN_EXPIRE_MINUTES)).timestamp() as usize;
    let refresh_exp = (now + Duration::days(REFRESH_TOKEN_EXPIRE_DAYS)).timestamp() as usize;

    let access_claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        token_type: "access".to_string(),
        exp: access_exp,
    };

    let refresh_claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        token_type: "refresh".to_string(),
        exp: refresh_exp,
    };

    let header = Header::new(Algorithm::HS256);

    let access_token = encode(
        &header,
        &access_claims,
        &EncodingKey::from_secret(JWT_SECRET),
    )
    .map_err(|_e| HttpResponse::InternalServerError().finish())?;

    let refresh_token = encode(
        &header,
        &refresh_claims,
        &EncodingKey::from_secret(JWT_SECRET),
    )
    .map_err(|_e| HttpResponse::InternalServerError().finish())?;

    Ok(LoginToken {
        token: access_token,
        refresh_token,
    })
}

pub async fn get_user_info(req: HttpRequest, db: web::Data<Arc<Database>>) -> impl Responder {
    // 这里的Claims是由AuthMiddleware注入的
    if let Some(claims) = req.extensions().get::<Claims>() {
        match db.get_user_info(&claims.username) {
            Ok(Some((id, username_in_db, _email))) => {
                let user_info = build_user_info(id, &username_in_db);
                let response = Response {
                    code: CODE_SUCCESS.to_string(),
                    msg: "获取用户信息成功".to_string(),
                    data: Some(serde_json::json!(user_info)),
                };
                HttpResponse::Ok().json(response)
            }
            Ok(None) => {
                let response = Response {
                    code: CODE_USER_NOT_FOUND.to_string(),
                    msg: "用户不存在".to_string(),
                    data: None,
                };
                HttpResponse::Ok().json(response)
            }
            Err(_e) => {
                let response = Response {
                    code: "500".to_string(),
                    msg: "数据库错误".to_string(),
                    data: None,
                };
                HttpResponse::Ok().json(response)
            }
        }
    } else {
        // 如果没有Claims，说明中间件未生效或未正确配置，或者请求未经过认证
        // 理论上AuthMiddleware会拦截无效请求，这里只是兜底
        let response = Response {
            code: CODE_TOKEN_INVALID.to_string(),
            msg: "未授权访问".to_string(),
            data: None,
        };
        HttpResponse::Unauthorized().json(response)
    }
}

// 处理登录请求
pub async fn login(
    login_data: web::Json<LoginRequest>,
    db: web::Data<Arc<Database>>,
) -> impl Responder {
    let username = &login_data.username;
    let password = &login_data.password;
    log::info!("login attempt for user: {}", username);

    match db.get_user_info(username) {
        Ok(Some((id, username_in_db, _email))) => {
            match db.validate_user(&username_in_db, password) {
                Ok(true) => match generate_tokens(id, &username_in_db) {
                    Ok(tokens) => {
                        log::info!("login success for user: {}", username_in_db);
                        let response = Response {
                            code: CODE_SUCCESS.to_string(),
                            msg: "登录成功".to_string(),
                            data: Some(serde_json::json!(tokens)),
                        };
                        HttpResponse::Ok().json(response)
                    }
                    Err(err_resp) => err_resp,
                },
                Ok(false) => {
                    log::warn!("login failed for user: {} (invalid credentials)", username);
                    let response = Response {
                        code: CODE_INVALID_CREDENTIALS.to_string(),
                        msg: "用户名或密码错误".to_string(),
                        data: None,
                    };
                    HttpResponse::Ok().json(response)
                }
                Err(_e) => {
                    let response = Response {
                        code: "500".to_string(),
                        msg: "验证用户失败".to_string(),
                        data: None,
                    };
                    HttpResponse::Ok().json(response)
                }
            }
        }
        Ok(None) => {
            log::warn!("login failed, user not found: {}", username);
            let response = Response {
                code: CODE_USER_NOT_FOUND.to_string(),
                msg: "用户不存在".to_string(),
                data: None,
            };
            HttpResponse::Ok().json(response)
        }
        Err(_e) => {
            let response = Response {
                code: "500".to_string(),
                msg: "数据库错误".to_string(),
                data: None,
            };
            HttpResponse::Ok().json(response)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ThemeConfigRequest {
    pub theme_config: String,
}

// 获取用户主题配置
pub async fn get_user_theme_config_handler(
    req: HttpRequest,
    db: web::Data<Arc<Database>>,
) -> impl Responder {
    if let Some(claims) = req.extensions().get::<Claims>() {
        match db.get_user_theme_config(&claims.username) {
            Ok(Some(config)) => {
                let response = Response {
                    code: CODE_SUCCESS.to_string(),
                    msg: "获取主题配置成功".to_string(),
                    data: Some(serde_json::json!({ "themeConfig": config })),
                };
                HttpResponse::Ok().json(response)
            }
            Ok(None) => {
                let response = Response {
                    code: CODE_SUCCESS.to_string(),
                    msg: "未找到主题配置".to_string(),
                    data: None,
                };
                HttpResponse::Ok().json(response)
            }
            Err(e) => {
                let response = Response {
                    code: "500".to_string(),
                    msg: format!("数据库错误: {}", e),
                    data: None,
                };
                HttpResponse::Ok().json(response)
            }
        }
    } else {
        HttpResponse::Unauthorized().finish()
    }
}

// 更新用户主题配置
pub async fn update_user_theme_config_handler(
    req: HttpRequest,
    body: web::Json<ThemeConfigRequest>,
    db: web::Data<Arc<Database>>,
) -> impl Responder {
    if let Some(claims) = req.extensions().get::<Claims>() {
        match db.update_user_theme_config(&claims.username, &body.theme_config) {
            Ok(_) => {
                let response = Response {
                    code: CODE_SUCCESS.to_string(),
                    msg: "更新主题配置成功".to_string(),
                    data: None,
                };
                HttpResponse::Ok().json(response)
            }
            Err(e) => {
                let response = Response {
                    code: "500".to_string(),
                    msg: format!("数据库错误: {}", e),
                    data: None,
                };
                HttpResponse::Ok().json(response)
            }
        }
    } else {
        HttpResponse::Unauthorized().finish()
    }
}

pub async fn refresh_token(
    payload: web::Json<serde_json::Value>,
    db: web::Data<Arc<Database>>,
) -> impl Responder {
    let refresh_token = match payload.get("refreshToken").and_then(|v| v.as_str()) {
        Some(t) => t.to_string(),
        None => {
            let response = Response {
                code: CODE_REFRESH_TOKEN_INVALID.to_string(),
                msg: "缺少refreshToken".to_string(),
                data: None,
            };
            return HttpResponse::BadRequest().json(response);
        }
    };

    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    validation.leeway = 0; // Strict expiration check

    match decode::<Claims>(
        &refresh_token,
        &DecodingKey::from_secret(JWT_SECRET),
        &validation,
    ) {
        Ok(data) => {
            if data.claims.token_type != "refresh" {
                let response = Response {
                    code: CODE_REFRESH_TOKEN_INVALID.to_string(),
                    msg: "refreshToken无效".to_string(),
                    data: None,
                };
                return HttpResponse::Unauthorized().json(response);
            }

            // 可选：检查用户是否仍然存在
            if let Ok(Some((id, username, _email))) = db.get_user_info(&data.claims.username) {
                match generate_tokens(id, &username) {
                    Ok(tokens) => {
                        log::info!("refresh token success for user: {}", username);
                        let response = Response {
                            code: CODE_SUCCESS.to_string(),
                            msg: "刷新令牌成功".to_string(),
                            data: Some(serde_json::json!(tokens)),
                        };
                        HttpResponse::Ok().json(response)
                    }
                    Err(err_resp) => err_resp,
                }
            } else {
                let response = Response {
                    code: CODE_USER_NOT_FOUND.to_string(),
                    msg: "用户不存在".to_string(),
                    data: None,
                };
                HttpResponse::Ok().json(response)
            }
        }
        Err(err) => {
            log::warn!("refresh token failed: {}", err);
            if matches!(
                err.kind(),
                jsonwebtoken::errors::ErrorKind::ExpiredSignature
            ) {
                let response = Response {
                    code: CODE_REFRESH_TOKEN_INVALID.to_string(),
                    msg: "refreshToken已过期".to_string(),
                    data: None,
                };
                HttpResponse::Unauthorized().json(response)
            } else {
                let response = Response {
                    code: CODE_REFRESH_TOKEN_INVALID.to_string(),
                    msg: "refreshToken无效".to_string(),
                    data: None,
                };
                HttpResponse::Unauthorized().json(response)
            }
        }
    }
}

// 健康检查端点（用于测试服务是否正常运行）
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
}
