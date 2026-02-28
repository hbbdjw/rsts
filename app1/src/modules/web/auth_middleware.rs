use std::future::{Ready, ready};
use std::rc::Rc;

use crate::modules::web::auth_utils::{CODE_TOKEN_EXPIRED, CODE_TOKEN_INVALID, verify_token};
use crate::modules::web::models::Response;
use actix_web::{
    Error, HttpMessage,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
};
use futures::future::LocalBoxFuture;

// There are two steps in middleware processing.
// 1. Middleware initialization, middleware factory gets called with
//    next service in chain as parameter.
// 2. Middleware's call method gets called with normal request.
pub struct AuthMiddleware;

impl AuthMiddleware {
    pub fn new() -> Self {
        AuthMiddleware
    }
}

// Middleware factory is `Transform` trait
impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddlewareMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct AuthMiddlewareMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &self,
        ctx: &mut core::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let svc = self.service.clone();

        Box::pin(async move {
            // Check for Authorization header
            let auth_header = req.headers().get("Authorization");

            let token = match auth_header {
                Some(header) => match header.to_str() {
                    Ok(header_str) => {
                        if header_str.starts_with("Bearer ") {
                            &header_str[7..]
                        } else {
                            ""
                        }
                    }
                    Err(_) => "",
                },
                None => "",
            };

            if token.is_empty() {
                log::warn!("Access denied: No token provided");
                let resp = Response {
                    code: CODE_TOKEN_INVALID.to_string(),
                    msg: "未提供访问令牌".to_string(),
                    data: None,
                };
                return Err(actix_web::error::ErrorUnauthorized(
                    serde_json::to_string(&resp).unwrap(),
                ));
            }

            match verify_token(token) {
                Ok(claims) => {
                    if claims.token_type != "access" {
                        log::warn!(
                            "Access denied: Invalid token type for user {}",
                            claims.username
                        );
                        let resp = Response {
                            code: CODE_TOKEN_INVALID.to_string(),
                            msg: "访问令牌无效".to_string(),
                            data: None,
                        };
                        return Err(actix_web::error::ErrorUnauthorized(
                            serde_json::to_string(&resp).unwrap(),
                        ));
                    }
                    // Insert claims into request extensions for handlers to use
                    req.extensions_mut().insert(claims);

                    let res = svc.call(req).await?;
                    Ok(res)
                }
                Err(err) => {
                    log::warn!("Access denied: Token verification failed - {}", err);
                    if matches!(
                        err.kind(),
                        jsonwebtoken::errors::ErrorKind::ExpiredSignature
                    ) {
                        let resp = Response {
                            code: CODE_TOKEN_EXPIRED.to_string(),
                            msg: "访问令牌已过期".to_string(),
                            data: None,
                        };
                        Err(actix_web::error::ErrorUnauthorized(
                            serde_json::to_string(&resp).unwrap(),
                        ))
                    } else {
                        let resp = Response {
                            code: CODE_TOKEN_INVALID.to_string(),
                            msg: "访问令牌无效".to_string(),
                            data: None,
                        };
                        Err(actix_web::error::ErrorUnauthorized(
                            serde_json::to_string(&resp).unwrap(),
                        ))
                    }
                }
            }
        })
    }
}
