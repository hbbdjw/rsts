use actix_web::{
    Error, FromRequest, HttpResponse,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::Method,
    web,
};
use futures::future::{Ready, ok};
use futures::task::{Context, Poll};
use regex;
use reqwest;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// 导入日志宏
use crate::log_debug;
use crate::log_error;
use crate::log_info;
use crate::log_warn;

// 定义拦截器规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterceptorRule {
    pub path_prefix: String,
    pub target_url: String,
    pub pattern: String,
}

// 定义代理中间件
pub struct ProxyMiddleware {
    // 用于跟踪转发深度的原子计数器
    pub forward_depth: Arc<AtomicUsize>,
    // 最大转发深度
    pub max_forward_depth: usize,
}

impl ProxyMiddleware {
    pub fn new() -> Self {
        ProxyMiddleware {
            forward_depth: Arc::new(AtomicUsize::new(0)),
            max_forward_depth: 5, // 默认最大转发深度为5
        }
    }

    // 检查请求是否应该被转发
    fn should_forward(&self, path: &str, rules: &[InterceptorRule]) -> Option<InterceptorRule> {
        for rule in rules {
            // 检查路径前缀是否匹配
            if path.starts_with(&rule.path_prefix) {
                // 如果配置了正则表达式，还需要匹配正则表达式
                if !rule.pattern.is_empty() {
                    if let Ok(regex) = regex::Regex::new(&rule.pattern) {
                        if regex.is_match(path) {
                            log_debug!("匹配到规则: {:?}", rule);
                            return Some(rule.clone());
                        }
                    } else {
                        log_error!("无效的正则表达式: {}", rule.pattern);
                    }
                } else {
                    // 没有配置正则表达式，仅匹配路径前缀
                    log_debug!("匹配到规则: {:?}", rule);
                    return Some(rule.clone());
                }
            }
        }
        log_debug!("未匹配到任何规则，继续正常处理");
        None
    }
}

// 实现Transform trait，用于将中间件应用到服务
impl<S, B> Transform<S, ServiceRequest> for ProxyMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static + actix_web::body::MessageBody,
{
    type Response = ServiceResponse;
    type Error = Error;
    type InitError = ();
    type Transform = ProxyService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(ProxyService {
            service: service,
            client: reqwest::Client::new(),
            forward_depth: self.forward_depth.clone(),
            max_forward_depth: self.max_forward_depth,
        })
    }
}

// 代理服务
pub struct ProxyService<S> {
    service: S,
    client: reqwest::Client,
    // 用于跟踪转发深度的原子计数器
    forward_depth: Arc<AtomicUsize>,
    // 最大转发深度
    max_forward_depth: usize,
}

// 定义用于标记转发请求的自定义头
const X_FORWARDED_BY_RSTS: &str = "x-forwarded-by-rsts";
const X_FORWARD_DEPTH: &str = "x-forward-depth";

// 实现Service trait，处理请求
impl<S, B> Service<ServiceRequest> for ProxyService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static + actix_web::body::MessageBody,
{
    type Response = ServiceResponse;
    type Error = Error;
    type Future =
        Pin<Box<dyn futures::Future<Output = Result<Self::Response, Self::Error>> + 'static>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // 获取配置
        let config = match req.app_data::<web::Data<crate::modules::config::config::Config>>() {
            Some(config) => config.get_ref().clone(),
            None => {
                log_error!("Failed to get configuration from app_data");
                return Box::pin(async {
                    Ok(req.into_response(
                        HttpResponse::InternalServerError().body("Failed to load configuration"),
                    ))
                });
            }
        };

        // 检查是否启用了代理
        if !config.proxy.enable {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_boxed_body())
            });
        }
        // 获取请求路径
        let path = req.path().to_string();
        // 记录请求方法
        let method = req.method().clone();
        log_info!("请求: {} {}", method.as_str(), path);

        // 检查是否已经是转发请求（通过自定义请求头识别）
        if req.headers().contains_key(X_FORWARDED_BY_RSTS) {
            log_warn!("检测到该请求已经是转发请求，避免循环转发");
            // 不进行转发，避免循环
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_boxed_body())
            });
        }

        // 检查是否应该转发请求
        let proxy_middleware = ProxyMiddleware::new();
        // 使用config模块中的InterceptorRule
        let rules: Vec<InterceptorRule> = config
            .proxy
            .rules
            .iter()
            .map(|r| InterceptorRule {
                path_prefix: r.path_prefix.clone(),
                target_url: r.target_url.clone(),
                pattern: r.pattern.clone().unwrap_or_default(),
            })
            .collect();

        log_debug!("规则数量: {}", rules.len());

        if let Some(rule) = proxy_middleware.should_forward(&path, rules.as_slice()) {
            // 检查当前转发深度
            let current_depth = self.forward_depth.load(Ordering::Relaxed);
            if current_depth >= self.max_forward_depth {
                log_warn!(
                    "达到最大转发深度({}/{})，停止转发以避免循环",
                    current_depth,
                    self.max_forward_depth
                );
                // 不进行转发，避免循环
                let fut = self.service.call(req);
                return Box::pin(async move {
                    let res = fut.await?;
                    Ok(res.map_into_boxed_body())
                });
            }

            // 检查是否存在循环转发风险（基于主机名）
            let current_host = req.connection_info().host().to_string();
            if rule.target_url.contains(&current_host) {
                log_warn!(
                    "检测到潜在的循环转发风险: 当前主机({}) 包含在目标URL({})中",
                    current_host,
                    rule.target_url
                );
                // 不进行转发，避免循环
                let fut = self.service.call(req);
                return Box::pin(async move {
                    let res = fut.await?;
                    Ok(res.map_into_boxed_body())
                });
            }

            // 构建目标URL - 确保URL格式正确
            let base_url = rule.target_url.trim_end_matches('/');
            let path_without_prefix = path.strip_prefix(&rule.path_prefix).unwrap_or(&path);

            // 智能构建URL，避免在无路径和查询参数时添加多余的斜杠和问号
            let target_url = if path_without_prefix.is_empty() {
                if req.query_string().is_empty() {
                    base_url.to_string()
                } else {
                    format!("{}?{}", base_url, req.query_string())
                }
            } else {
                if req.query_string().is_empty() {
                    format!(
                        "{}/{}",
                        base_url,
                        path_without_prefix.trim_start_matches('/')
                    )
                } else {
                    format!(
                        "{}/{}?{}",
                        base_url,
                        path_without_prefix.trim_start_matches('/'),
                        req.query_string()
                    )
                }
            };

            log_info!(
                "目标URL: {} ,原始规则target_url: {},路径前缀: {} ,去除前缀后的路径: {}",
                target_url,
                rule.target_url,
                rule.path_prefix,
                path_without_prefix
            );
            // 获取请求头
            let headers = req.headers().clone();
            // 记录内容类型头
            if let Some(content_type) = headers.get(actix_web::http::header::CONTENT_TYPE) {
                if let Ok(content_type_str) = content_type.to_str() {
                    log_info!("Content-Type: {}", content_type_str);
                }
            }

            // 获取请求体
            let (req_parts, mut req_body) = req.into_parts();
            let req_parts_clone = req_parts.clone();

            // 读取请求体
            let body_future = async move {
                // 使用actix_web内置的方法读取请求体
                match actix_web::web::Bytes::from_request(&req_parts_clone, &mut req_body).await {
                    Ok(bytes) => {
                        log_info!("请求体大小: {} 字节", bytes.len());
                        Ok(bytes.to_vec())
                    }
                    Err(e) => {
                        log_error!("读取请求体失败: {:?}", e);
                        Err(Error::from(actix_web::error::ErrorInternalServerError(
                            "Failed to read request body",
                        )))
                    }
                }
            };

            // 克隆客户端、目标URL和转发深度计数器以在异步块中使用
            let client = self.client.clone();
            let target_url = target_url.clone();
            let forward_depth = self.forward_depth.clone();
            let method = method.clone();

            // 执行转发请求
            Box::pin(async move {
                // 增加转发深度计数器
                let new_depth = forward_depth.fetch_add(1, Ordering::Relaxed) + 1;
                log_info!("当前转发深度: {}", new_depth);

                // 等待请求体读取完成
                let body_bytes = match body_future.await {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        // 还原转发深度计数器
                        forward_depth.fetch_sub(1, Ordering::Relaxed);
                        return Ok(ServiceResponse::from_err(
                            actix_web::error::ErrorInternalServerError(
                                "Failed to read request body",
                            ),
                            req_parts.clone(),
                        ));
                    }
                };

                // 构建转发请求
                log_info!("使用{}方法构建请求", method.as_str());
                let mut request_builder = match method {
                    Method::GET => client.get(&target_url),
                    Method::POST => client.post(&target_url),
                    Method::PUT => client.put(&target_url),
                    Method::DELETE => client.delete(&target_url),
                    Method::PATCH => client.patch(&target_url),
                    Method::HEAD => client.head(&target_url),
                    Method::OPTIONS => client.request(reqwest::Method::OPTIONS, &target_url),
                    _ => {
                        // 还原转发深度计数器
                        forward_depth.fetch_sub(1, Ordering::Relaxed);
                        log_warn!("不支持的HTTP方法: {}", method.as_str());
                        return Ok(ServiceResponse::from_err(
                            actix_web::error::ErrorMethodNotAllowed("Method not allowed"),
                            req_parts.clone(),
                        ));
                    }
                };

                // 添加请求头
                for (name, value) in headers.iter() {
                    // 避免添加连接相关的头，这些由reqwest自动处理
                    let header_name = name.as_str().to_lowercase();
                    if !header_name.contains("connection")
                        && !header_name.contains("content-length")
                        && !header_name.contains("host")
                    {
                        if let Ok(value_str) = value.to_str() {
                            log_debug!("转发请求头: {}: {}", name.as_str(), value_str);
                        }
                        request_builder = request_builder.header(name.as_str(), value.as_bytes());
                    }
                }

                // 添加自定义转发标记头
                request_builder = request_builder.header(X_FORWARDED_BY_RSTS, "rsts-proxy");
                request_builder = request_builder.header(X_FORWARD_DEPTH, new_depth.to_string());
                // 添加原始主机头
                if let Some(host) = req_parts.headers().get(actix_web::http::header::HOST) {
                    request_builder = request_builder.header("X-Forwarded-Host", host.as_bytes());
                }

                // 添加请求体
                if !body_bytes.is_empty() {
                    log_info!("添加请求体，大小: {} 字节", body_bytes.len());
                    // 尝试记录请求体内容（仅用于调试）
                    let body_str = String::from_utf8_lossy(&body_bytes);
                    log_debug!("请求体内容: {}", body_str);
                    request_builder = request_builder.body(body_bytes);
                }

                // 发送请求
                log_info!("发送请求到: {}", target_url);
                let response = match request_builder.send().await {
                    Ok(res) => {
                        log_info!("收到响应，状态码: {}", res.status());
                        res
                    }
                    Err(e) => {
                        // 还原转发深度计数器
                        forward_depth.fetch_sub(1, Ordering::Relaxed);
                        log_error!("请求失败: {}", e);
                        return Ok(ServiceResponse::from_err(
                            actix_web::error::ErrorInternalServerError(format!(
                                "Failed to forward request: {}",
                                e
                            )),
                            req_parts.clone(),
                        ));
                    }
                };

                // 构建响应
                let status = response.status();
                let headers = response.headers().clone();
                let body_bytes = match response.bytes().await {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        // 还原转发深度计数器
                        forward_depth.fetch_sub(1, Ordering::Relaxed);
                        return Ok(ServiceResponse::from_err(
                            actix_web::error::ErrorInternalServerError(
                                "Failed to read response body",
                            ),
                            req_parts.clone(),
                        ));
                    }
                };

                // 创建HTTP响应
                let mut http_response = HttpResponse::build(
                    actix_web::http::StatusCode::from_u16(status.as_u16()).unwrap(),
                );
                for (name, value) in headers.iter() {
                    // 避免添加连接相关的头，这些由actix自动处理
                    if !name.as_str().to_lowercase().contains("connection") {
                        http_response.insert_header((name.as_str(), value.as_bytes()));
                    }
                }
                let http_response = http_response.body(body_bytes);

                // 创建服务响应
                let service_response = ServiceResponse::new(req_parts.clone(), http_response);

                // 还原转发深度计数器
                forward_depth.fetch_sub(1, Ordering::Relaxed);

                Ok(service_response)
            })
        } else {
            // 不需要转发的请求，继续正常处理
            let fut = self.service.call(req);
            Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_boxed_body())
            })
        }
    }
}
