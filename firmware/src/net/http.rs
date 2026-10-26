use crate::ota::OtaManager;
use matrix_control::api::ApiContext;
#[cfg(target_os = "espidf")]
use matrix_control::api::{handle_api_request, HttpResponse};
use std::sync::{Arc, Mutex};

pub const MAX_API_BODY_BYTES: usize = 4096;

pub struct FirmwareHttpServer {
    #[cfg(target_os = "espidf")]
    _server: esp_idf_svc::http::server::EspHttpServer<'static>,
}

#[cfg(target_os = "espidf")]
fn memory_is_low() -> bool {
    unsafe {
        let free = esp_idf_sys::esp_get_free_heap_size();
        let largest_block =
            esp_idf_sys::heap_caps_get_largest_free_block(esp_idf_sys::MALLOC_CAP_8BIT);
        // Shed incoming dynamic API requests when free heap drops below 16 KB or largest
        // allocatable block drops below 8 KB. This protects the ESP32 from OOM aborts.
        free < 16 * 1024 || largest_block < 8 * 1024
    }
}

impl FirmwareHttpServer {
    #[cfg(target_os = "espidf")]
    pub fn start(
        api_ctx: Arc<ApiContext>,
        ota_manager: Arc<Mutex<OtaManager>>,
    ) -> Result<Self, String> {
        use esp_idf_svc::http::server::{Configuration, EspHttpServer};
        use esp_idf_svc::http::Method;

        let config = Configuration {
            // API JSON parsing plus synchronous command acknowledgement share
            // this native task stack (independent of Rust pthread defaults).
            stack_size: 16384,
            max_uri_handlers: 16,
            uri_match_wildcard: true,
            ..Default::default()
        };

        let mut server = EspHttpServer::new(&config).map_err(|e| e.to_string())?;

        // 1. Root /
        server
            .fn_handler("/", Method::Get, move |req| {
                use embedded_svc::io::Write;
                // Serve flash-resident HTML directly; a copied Vec needs a large
                // contiguous heap block precisely when networking is active.
                let mut response = req.into_response(
                    200,
                    None,
                    &[
                        ("Content-Type", "text/html; charset=utf-8"),
                        ("Connection", "close"),
                    ],
                )?;
                for chunk in matrix_control::WEB_UI.as_bytes().chunks(1024) {
                    response.write_all(chunk)?;
                }
                Ok::<(), anyhow::Error>(())
            })
            .map_err(|e| e.to_string())?;

        // Stream the flash-resident browser renderer without a large heap copy.
        server
            .fn_handler("/preview.wasm.gz", Method::Get, move |req| {
                use embedded_svc::io::Write;
                let mut response = req.into_response(
                    200,
                    None,
                    &[
                        ("Content-Type", "application/gzip"),
                        ("Cache-Control", "no-cache"),
                        ("Connection", "close"),
                    ],
                )?;
                for chunk in matrix_control::api::PREVIEW_WASM.chunks(1024) {
                    response.write_all(chunk)?;
                }
                Ok::<(), anyhow::Error>(())
            })
            .map_err(|e| e.to_string())?;

        // 2. Wildcard GET /api/*
        let ctx = api_ctx.clone();
        server
            .fn_handler("/api/*", Method::Get, move |req| {
                if memory_is_low() {
                    return send_response(
                        req,
                        HttpResponse::err_json(503, "device low on memory, shedding load"),
                    );
                }
                let uri = req.uri().to_string();
                if matches!(uri.as_str(), "/api/effects" | "/api/catalog") {
                    use embedded_svc::io::Write;
                    let catalog = ctx.hub.catalog_json();
                    let mut response = req.into_response(
                        200,
                        None,
                        &[
                            ("Content-Type", "application/json"),
                            ("Connection", "close"),
                        ],
                    )?;
                    for chunk in catalog.chunks(1024) {
                        response.write_all(chunk)?;
                    }
                    return Ok::<(), anyhow::Error>(());
                }
                let resp = handle_api_request(&ctx, "GET", &uri, &[]);
                send_response(req, resp)
            })
            .map_err(|e| e.to_string())?;

        // 3. Wildcard POST /api/* with bounded reads
        let ctx = api_ctx.clone();
        server
            .fn_handler("/api/*", Method::Post, move |mut req| {
                if memory_is_low() {
                    return send_response(
                        req,
                        HttpResponse::err_json(503, "device low on memory, shedding load"),
                    );
                }
                let uri = req.uri().to_string();
                let mut body = Vec::new();
                let mut buf = [0u8; 512];
                let mut total_read = 0;
                let mut payload_too_large = false;

                loop {
                    let n = match req.read(&mut buf) {
                        Ok(n) => n,
                        Err(_) => {
                            return send_response(
                                req,
                                HttpResponse::err_json(400, "request body transport failed"),
                            )
                        }
                    };
                    if n == 0 {
                        break;
                    }
                    total_read += n;
                    if total_read > MAX_API_BODY_BYTES {
                        payload_too_large = true;
                        break;
                    }
                    body.extend_from_slice(&buf[..n]);
                }

                if payload_too_large {
                    let resp = HttpResponse::err_json(413, "payload too large");
                    return send_response(req, resp);
                }

                let resp = handle_api_request(&ctx, "POST", &uri, &body);
                send_response(req, resp)
            })
            .map_err(|e| e.to_string())?;

        // 4. Safe streaming OTA update endpoint /update
        let ota_mgr = ota_manager.clone();
        server
            .fn_handler("/update", Method::Post, move |mut req| {
                let mut ota = match ota_mgr.lock() {
                    Ok(guard) => guard,
                    Err(_) => {
                        let resp = HttpResponse::err_json(500, "OTA lock failed");
                        return send_response(req, resp);
                    }
                };

                let length = req
                    .header("Content-Length")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);
                let partition_size = match OtaManager::update_partition_size() {
                    Ok(size) => size,
                    Err(error) => return send_response(req, HttpResponse::err_json(503, &error)),
                };
                if let Err(error) = OtaManager::validate_image_length(length, partition_size) {
                    return send_response(req, HttpResponse::err_json(413, &error));
                }
                let mut buf = [0u8; 1024];
                let first = match req.read(&mut buf) {
                    Ok(n) if n > 0 => n,
                    _ => {
                        return send_response(
                            req,
                            HttpResponse::err_json(400, "cannot read image header"),
                        )
                    }
                };
                if let Err(error) = OtaManager::validate_header(&buf[..first]) {
                    return send_response(req, HttpResponse::err_json(400, &error));
                }
                if let Err(error) = ota.begin() {
                    ota.abort();
                    return send_response(req, HttpResponse::err_json(500, &error));
                }
                let mut total_bytes = first;
                let transfer = (|| -> Result<(), String> {
                    ota.write_chunk(&buf[..first])?;
                    while total_bytes < length {
                        let remaining = (length - total_bytes).min(buf.len());
                        let n = req
                            .read(&mut buf[..remaining])
                            .map_err(|_| "image transport failed")?;
                        if n == 0 {
                            return Err("truncated image body".into());
                        }
                        ota.write_chunk(&buf[..n])?;
                        total_bytes += n;
                    }
                    ota.finish()
                })();
                if let Err(error) = transfer {
                    ota.abort();
                    return send_response(req, HttpResponse::err_json(400, &error));
                }

                let resp = HttpResponse::ok_json(serde_json::json!({
                    "ok": true,
                    "bytes": total_bytes,
                    "restart": true
                }));
                let res = send_response(req, resp);
                OtaManager::schedule_restart();
                res
            })
            .map_err(|e| e.to_string())?;

        log::info!("Firmware HTTP server listening on port 80");
        Ok(Self { _server: server })
    }

    #[cfg(not(target_os = "espidf"))]
    pub fn start(
        _api_ctx: Arc<ApiContext>,
        _ota_manager: Arc<Mutex<OtaManager>>,
    ) -> Result<Self, String> {
        log::info!("Firmware HTTP server started (host simulator mode)");
        Ok(Self {})
    }
}

#[cfg(target_os = "espidf")]
fn send_response<'a>(
    req: esp_idf_svc::http::server::Request<
        &'a mut esp_idf_svc::http::server::EspHttpConnection<'_>,
    >,
    resp: HttpResponse,
) -> anyhow::Result<()> {
    use embedded_svc::io::Write;
    let mut response = req.into_response(
        resp.status,
        None,
        &[("Content-Type", resp.content_type), ("Connection", "close")],
    )?;
    response.write_all(&resp.body)?;
    Ok(())
}
