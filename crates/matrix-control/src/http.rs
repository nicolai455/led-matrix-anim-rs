#[cfg(feature = "host")]
use crate::api::{handle_api_request, ApiContext};
#[cfg(feature = "host")]
use std::io::Read;
#[cfg(feature = "host")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "host")]
use std::sync::Arc;
#[cfg(feature = "host")]
use std::thread;
#[cfg(feature = "host")]
use tiny_http::{Header, Response, Server};

#[cfg(feature = "host")]
pub struct HttpServerHandle {
    shutdown: Arc<AtomicBool>,
}

#[cfg(feature = "host")]
impl HttpServerHandle {
    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

#[cfg(feature = "host")]
pub fn start_http_server(
    listen_addr: &str,
    ctx: Arc<ApiContext>,
) -> Result<HttpServerHandle, String> {
    let server = Server::http(listen_addr)
        .map_err(|e| format!("failed to bind HTTP server to {listen_addr}: {e}"))?;

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_thread = shutdown.clone();

    thread::Builder::new()
        .name("matrix-http".to_string())
        .spawn(move || {
            while !shutdown_thread.load(Ordering::Relaxed) {
                // recv_timeout allows checking shutdown flag
                match server.recv_timeout(std::time::Duration::from_millis(200)) {
                    Ok(Some(mut request)) => {
                        let method = request.method().as_str().to_string();
                        let url = request.url().to_string();
                        // Strip query parameters for routing if needed
                        let path = url.split('?').next().unwrap_or(&url);

                        let mut body = Vec::new();
                        let read =
                            std::io::Read::take(request.as_reader(), 4097).read_to_end(&mut body);
                        let res = if read.is_err() {
                            crate::api::HttpResponse::err_json(400, "request body read failed")
                        } else if body.len() > 4096 {
                            crate::api::HttpResponse::err_json(
                                413,
                                "request body exceeds 4096 bytes",
                            )
                        } else {
                            handle_api_request(&ctx, &method, path, &body)
                        };

                        let mut headers = vec![
                            Header::from_bytes(&b"Content-Type"[..], res.content_type.as_bytes())
                                .unwrap(),
                            Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..])
                                .unwrap(),
                        ];
                        if res.content_type.starts_with("text/html") {
                            headers.push(
                                Header::from_bytes(
                                    &b"Cache-Control"[..],
                                    &b"no-cache, no-store, must-revalidate"[..],
                                )
                                .unwrap(),
                            );
                        }

                        let tiny_res = Response::new(
                            res.status.into(),
                            headers,
                            std::io::Cursor::new(res.body),
                            None,
                            None,
                        );

                        let _ = request.respond(tiny_res);
                    }
                    Ok(None) => {}
                    Err(_) => break,
                }
            }
        })
        .map_err(|e| format!("failed to spawn HTTP server thread: {e}"))?;

    Ok(HttpServerHandle { shutdown })
}
