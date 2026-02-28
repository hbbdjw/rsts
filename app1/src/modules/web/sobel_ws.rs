use crate::modules::operators::sobel::SobelOperator;
use actix::prelude::*;
use actix_web::{Error, HttpRequest, HttpResponse, web};
use actix_web_actors::ws;
use std::time::Instant;

/// WebSocket actor for Sobel operator
pub struct SobelWsSession {
    pub hb: Instant,
}

impl SobelWsSession {
    pub fn new() -> Self {
        Self { hb: Instant::now() }
    }
}

impl Actor for SobelWsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        // Start heartbeat if necessary, skipping for simplicity in this demo
    }
}

/// Handler for WebSocket messages
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for SobelWsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Pong(_)) => self.hb = Instant::now(),
            Ok(ws::Message::Binary(bin)) => {
                let data = bin.to_vec();

                // Offload the CPU-intensive Sobel operation to a blocking thread
                let fut = async move {
                    tokio::task::spawn_blocking(move || SobelOperator::apply_to_buffer(&data)).await
                };

                // Handle the result back in the actor's context
                let actor_future = actix::fut::wrap_future(fut).map(
                    |result, _, ctx: &mut ws::WebsocketContext<Self>| {
                        match result {
                            Ok(Ok(processed_data)) => {
                                ctx.binary(processed_data);
                            }
                            Ok(Err(e)) => {
                                // Log error but maybe don't send text to avoid breaking binary protocol expectations on client
                                eprintln!("Sobel processing error: {}", e);
                            }
                            Err(e) => {
                                eprintln!("Task join error: {}", e);
                            }
                        }
                    },
                );

                ctx.spawn(actor_future);
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => (),
        }
    }
}

/// Route handler
pub async fn sobel_ws_route(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    ws::start(SobelWsSession::new(), &req, stream)
}
