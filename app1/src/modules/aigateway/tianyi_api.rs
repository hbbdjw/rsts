use actix::prelude::*;
use actix_web::{Error, HttpRequest, HttpResponse, web};
use actix_web_actors::ws;
use bytes::{Bytes, BytesMut};
use futures::stream;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::time::{Duration, Instant};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);
const TIANYI_API_URL: &str = "https://wishub-x6.ctyun.cn/v1/chat/completions";
// 默认模型ID，如果客户端未提供则使用此ID
const DEFAULT_MODEL_ID: &str = "6d3a57c3a6fb465e968b604783b89eda";

/// WebSocket 消息结构
/// 客户端发送的消息应包含 api_key (可选) 和 payload (Tianyi API 请求体)
#[derive(Deserialize)]
struct TianyiWsMessage {
    /// API Key (Bearer Token), 如果未提供，需要在服务端配置或通过其他方式获取
    api_key: Option<String>,
    /// 其他字段将作为 payload 发送给 Tianyi API
    #[serde(flatten)]
    payload: Value,
}

pub struct TianyiSession {
    hb: Instant,
    client: Client,
    buf: BytesMut,
}

impl TianyiSession {
    pub fn new() -> Self {
        Self {
            hb: Instant::now(),
            client: Client::new(),
            buf: BytesMut::new(),
        }
    }

    /// 心跳检查
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                ctx.stop();
                return;
            }
            ctx.ping(b"");
        });
    }
}

impl Actor for TianyiSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
    }
}

impl StreamHandler<Result<Bytes, reqwest::Error>> for TianyiSession {
    fn handle(&mut self, item: Result<Bytes, reqwest::Error>, ctx: &mut Self::Context) {
        match item {
            Ok(bytes) => {
                self.buf.extend_from_slice(&bytes);
                match std::str::from_utf8(&self.buf) {
                    Ok(text) => {
                        ctx.text(text.to_string());
                        self.buf.clear();
                    }
                    Err(e) => {
                        let valid_len = e.valid_up_to();
                        if e.error_len().is_some() {
                            let text = String::from_utf8_lossy(&self.buf);
                            ctx.text(text.to_string());
                            self.buf.clear();
                        } else {
                            if valid_len > 0 {
                                let valid_bytes = self.buf.split_to(valid_len);
                                let text = unsafe { std::str::from_utf8_unchecked(&valid_bytes) };
                                ctx.text(text.to_string());
                            }
                        }
                    }
                }
            }
            Err(e) => {
                ctx.text(format!("{{\"error\": {{\"message\": \"Stream Error: {}\", \"type\": \"STREAM_ERROR\"}}}}", e));
                ctx.stop();
            }
        }
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for TianyiSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                let msg_text = text.to_string();
                let client = self.client.clone();

                // 异步处理请求
                let fut = async move {
                    // 解析客户端消息
                    let req_json: Result<TianyiWsMessage, _> = serde_json::from_str(&msg_text);

                    match req_json {
                        Ok(mut req_data) => {
                            let api_key = req_data.api_key.take();

                            // 检查 API Key
                            let api_key = match api_key {
                                Some(k) => k,
                                None => {
                                    return Err(serde_json::json!({
                                        "error": {
                                            "message": "api_key is required",
                                            "type": "PARAM_ERROR"
                                        }
                                    })
                                    .to_string());
                                }
                            };

                            // 处理 Payload
                            if let Some(obj) = req_data.payload.as_object_mut() {
                                // 如果未指定 model，使用默认值
                                if !obj.contains_key("model") {
                                    obj.insert(
                                        "model".to_string(),
                                        Value::String(DEFAULT_MODEL_ID.to_string()),
                                    );
                                }
                                // 开启流式输出
                                obj.insert("stream".to_string(), Value::Bool(true));
                            }

                            // 发送请求到 Tianyi API
                            let res = client
                                .post(TIANYI_API_URL)
                                .header("Authorization", format!("Bearer {}", api_key))
                                .header("Content-Type", "application/json")
                                .json(&req_data.payload)
                                .send()
                                .await;

                            match res {
                                Ok(response) => {
                                    if response.status().is_success() {
                                        Ok(response)
                                    } else {
                                        // 尝试读取错误响应体
                                        let status = response.status();
                                        let error_body = response.text().await.unwrap_or_default();
                                        Err(serde_json::json!({
                                            "error": {
                                                "message": format!("API Error: Status {}", status),
                                                "details": error_body,
                                                "type": "API_ERROR"
                                            }
                                        })
                                        .to_string())
                                    }
                                }
                                Err(e) => Err(serde_json::json!({
                                    "error": {
                                        "message": format!("Request Error: {}", e),
                                        "type": "NETWORK_ERROR"
                                    }
                                })
                                .to_string()),
                            }
                        }
                        Err(e) => Err(serde_json::json!({
                            "error": {
                                "message": format!("Invalid JSON: {}", e),
                                "type": "PARAM_ERROR"
                            }
                        })
                        .to_string()),
                    }
                };

                // 将异步结果发送回 Actor
                let fut = actix::fut::wrap_future(fut).map(
                    |result, _act, ctx: &mut ws::WebsocketContext<Self>| match result {
                        Ok(response) => {
                            let stream = stream::unfold(Some(response), |state| async move {
                                let mut resp = state?;
                                match resp.chunk().await {
                                    Ok(Some(chunk)) => Some((Ok(chunk), Some(resp))),
                                    Ok(None) => None,
                                    Err(e) => Some((Err(e), None)),
                                }
                            });
                            ctx.add_stream(stream);
                        }
                        Err(err_msg) => {
                            ctx.text(err_msg);
                        }
                    },
                );

                ctx.spawn(fut);
            }
            Ok(ws::Message::Binary(_)) => ctx.binary(&b"Binary not supported"[..]),
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

/// WebSocket 路由入口
pub async fn tianyi_ws_route(
    req: HttpRequest,
    stream: web::Payload,
) -> Result<HttpResponse, Error> {
    ws::start(TianyiSession::new(), &req, stream)
}
