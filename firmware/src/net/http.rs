use crate::ota::OtaManager;
use matrix_control::api::ApiContext;
#[cfg(target_os = "espidf")]
use matrix_control::api::{handle_api_request, HttpResponse};
use std::sync::{Arc, Mutex};

pub const MAX_API_BODY_BYTES: usize = 4096;

#[cfg(target_os = "espidf")]
type PeerRequest<'a, 'b> = esp_idf_svc::http::server::Request<&'a mut esp_idf_svc::http::server::EspHttpConnection<'b>>;

/// Everything the request guard needs, extracted once from the request.
#[cfg(target_os = "espidf")]
struct RequestFacts {
    origin: Option<String>,
    host: Option<String>,
    marker: Option<String>,
    token: Option<String>,
    peer: Option<std::net::Ipv4Addr>,
}

#[cfg(target_os = "espidf")]
fn request_facts(req: &mut PeerRequest<'_, '_>) -> (RequestFacts, Option<(std::net::Ipv4Addr, u8)>) {
    let (headers, _) = req.split();
    let origin = headers.header("Origin").map(str::to_owned);
    let host = headers.header("Host").map(str::to_owned);
    let marker = headers.header("X-Matrix-Control").map(str::to_owned);
    let token = headers
        .header(crate::setup_token::SETUP_TOKEN_HEADER)
        .map(str::to_owned);
    let peer = req
        .connection()
        .raw_connection()
        .ok()
        .and_then(|raw| raw.source_ipv4().ok());
    (
        RequestFacts {
            origin,
            host,
            marker,
            token,
            peer,
        },
        None,
    )
}

/// Classify the caller. `station_subnet` comes from the Wi-Fi worker, so a home
/// LAN client is recognised without touching the driver from the HTTP thread.
#[cfg(target_os = "espidf")]
fn control_scope(
    facts: &RequestFacts,
    station_subnet: Option<(std::net::Ipv4Addr, u8)>,
) -> crate::request_guard::ControlScope {
    crate::request_guard::classify_peer(facts.peer, station_subnet)
}

#[cfg(target_os = "espidf")]
fn trusted_request(
    req: &mut esp_idf_svc::http::server::Request<
        &mut esp_idf_svc::http::server::EspHttpConnection<'_>,
    >,
) -> bool {
    let (facts, _) = request_facts(req);
    crate::request_guard::trusted_management_peer(facts.peer)
        && crate::request_guard::trusted_control(
            facts.origin.as_deref(),
            facts.host.as_deref(),
            facts.marker.as_deref(),
        )
}

pub struct FirmwareHttpServer {
    #[cfg(target_os = "espidf")]
    _server: esp_idf_svc::http::server::EspHttpServer<'static>,
}

/// Conservative admission heuristic for allocating API requests, not an OOM
/// guarantee or a measured networking reservation. The internal floor is the
/// maximum buffered API body. The 1 KiB DMA floor is an uncalibrated, small
/// emergency reserve, not a measured DMA allocation requirement. These floors
/// are not claims that any allocation will actually succeed. Capability regions
/// can overlap and must never be summed. PSRAM cannot substitute for internal/DMA memory.
/// Cached state/catalog GETs, status, recovery, static assets, reboot and
/// streaming OTA bypass this policy so recovery remains possible.
/// Rejected API requests return HTTP 503 with a retryable memory-pressure error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HttpHeapPressure {
    pub internal_free: usize,
    pub internal_largest: usize,
    pub dma_free: usize,
    pub dma_largest: usize,
}

pub const DMA_PRESSURE_FLOOR_BYTES: usize = 1024;

pub fn shed_dynamic_api(heap: HttpHeapPressure) -> bool {
    heap.internal_free < MAX_API_BODY_BYTES
        || heap.internal_largest < MAX_API_BODY_BYTES
        || heap.dma_free < DMA_PRESSURE_FLOOR_BYTES
        || heap.dma_largest < DMA_PRESSURE_FLOOR_BYTES
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiAdmission {
    Allow,
    Shed,
}

/// Cached GETs stream existing bytes; status remains available for diagnostics.
/// Explicit recovery and OTA routes bypass the wildcard handlers.
pub fn api_admission(method: &str, path: &str, memory_low: bool) -> ApiAdmission {
    let exempt = matches!(
        (method, path),
        ("GET", "/api/state" | "/api/effects" | "/api/catalog" | "/api/status" | "/api/firmware" | "/api/recovery")
            | ("POST", "/api/recovery/retry" | "/api/reboot")
    );
    if memory_low && path.starts_with("/api/") && !exempt {
        ApiAdmission::Shed
    } else {
        ApiAdmission::Allow
    }
}

#[cfg(target_os = "espidf")]
fn memory_is_low() -> bool {
    use esp_idf_sys as idf;
    let internal = idf::MALLOC_CAP_INTERNAL | idf::MALLOC_CAP_8BIT;
    let dma = idf::MALLOC_CAP_INTERNAL | idf::MALLOC_CAP_DMA;
    // Samples are advisory and can change immediately after the calls.
    unsafe {
        shed_dynamic_api(HttpHeapPressure {
            internal_free: idf::heap_caps_get_free_size(internal),
            internal_largest: idf::heap_caps_get_largest_free_block(internal),
            dma_free: idf::heap_caps_get_free_size(dma),
            dma_largest: idf::heap_caps_get_largest_free_block(dma),
        })
    }
}

impl FirmwareHttpServer {
    #[cfg(target_os = "espidf")]
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        api_ctx: Arc<ApiContext>,
        ota_manager: Arc<Mutex<OtaManager>>,
        network_nvs: Arc<crate::nvs_store::FirmwareNvs>,
        wifi_state: Arc<crate::net::status::WifiState>,
        scan_cache: Arc<crate::net::scan::ScanCache>,
        setup_token: Arc<String>,
    ) -> Result<Self, String> {
        use esp_idf_svc::http::server::{Configuration, EspHttpServer};
        use esp_idf_svc::http::Method;

        let config = Configuration {
            // API JSON parsing plus synchronous command acknowledgement share
            // this native task stack (independent of Rust pthread defaults).
            stack_size: 16384,
            // 16 previous handlers + /api/network/scan.
            max_uri_handlers: 18,
            uri_match_wildcard: true,
            ..Default::default()
        };

        let mut server = EspHttpServer::new(&config).map_err(|e| {
            let caps = esp_idf_sys::MALLOC_CAP_INTERNAL | esp_idf_sys::MALLOC_CAP_8BIT;
            let (free, largest, minimum) = unsafe {
                (esp_idf_sys::heap_caps_get_free_size(caps),
                 esp_idf_sys::heap_caps_get_largest_free_block(caps),
                 esp_idf_sys::heap_caps_get_minimum_free_size(caps))
            };
            format!("{e}; HTTP stack={} internal free={free} largest={largest} minimum={minimum}", config.stack_size)
        })?;

        let network_nvs_get = Arc::clone(&network_nvs);
        let wifi_state_firmware = Arc::clone(&wifi_state);
        let setup_token_firmware = Arc::clone(&setup_token);
        server
            .fn_handler("/api/network", Method::Get, move |req| {
                let response = match crate::net::network_api::get_config(&network_nvs_get) {
                    Ok(value) => HttpResponse::ok_json(value),
                    Err(error) => HttpResponse::err_json(500, &error),
                };
                send_response(req, response)
            })
            .map_err(|error| error.to_string())?;

        // GET /api/network/scan never scans: it asks the Wi-Fi worker and reads
        // the cache. A cold cache returns an empty list with `stale: true`.
        let scan_cache_get = Arc::clone(&scan_cache);
        let wifi_state_scan = Arc::clone(&wifi_state);
        server
            .fn_handler("/api/network/scan", Method::Get, move |mut req| {
                if api_admission("GET", "/api/network/scan", memory_is_low()) == ApiAdmission::Shed {
                    return send_response(
                        req,
                        HttpResponse::err_json(503, "device low on memory, shedding load"),
                    );
                }
                let station_subnet = wifi_state_scan.station_subnet();
                let (facts, _) = request_facts(&mut req);
                let scope = control_scope(&facts, station_subnet);
                if scope == crate::request_guard::ControlScope::Untrusted {
                    return send_response(req, HttpResponse::err_json(403, "Untrusted scan request"));
                }
                scan_cache_get.request();
                let body = scan_cache_get.json(
                    std::time::Instant::now(),
                    wifi_state_scan.status().is_mixed(),
                );
                send_response(req, HttpResponse::ok_json(body))
            })
            .map_err(|error| error.to_string())?;

        let network_nvs_post = Arc::clone(&network_nvs);
        let ota_for_network = Arc::clone(&ota_manager);
        let wifi_state_post = Arc::clone(&wifi_state);
        let setup_token_post = Arc::clone(&setup_token);
        server
            .fn_handler("/api/network", Method::Post, move |mut req| {
                let (facts, _) = request_facts(&mut req);
                let scope = control_scope(&facts, wifi_state_post.station_subnet());
                let allowed = crate::request_guard::trusted_write(
                    scope,
                    facts.marker.as_deref(),
                    facts.origin.as_deref(),
                    facts.host.as_deref(),
                    facts.token.as_deref(),
                    setup_token_post.as_str(),
                );
                if !allowed {
                    let message = match scope {
                        crate::request_guard::ControlScope::LocalSubnet => {
                            "Setup token required for Wi-Fi changes from this network"
                        }
                        _ => "Untrusted control request",
                    };
                    return send_response(req, HttpResponse::err_json(403, message));
                }
                if api_admission("POST", "/api/network", memory_is_low()) == ApiAdmission::Shed {
                    return send_response(
                        req,
                        HttpResponse::err_json(503, "device low on memory, shedding load"),
                    );
                }
                let mut body = Vec::with_capacity(512);
                let mut chunk = [0u8; 512];
                loop {
                    let read = match req.read(&mut chunk) {
                        Ok(read) => read,
                        Err(_) => return send_response(req, HttpResponse::err_json(400, "request body transport failed")),
                    };
                    if read == 0 { break; }
                    if body.len() + read > MAX_API_BODY_BYTES {
                        return send_response(req, HttpResponse::err_json(413, "payload too large"));
                    }
                    body.extend_from_slice(&chunk[..read]);
                }
                let config = match crate::net::network_api::update_config(&network_nvs_post, &body) {
                    Ok(value) => value,
                    Err(error) => return send_response(req, HttpResponse::err_json(400, &error)),
                };
                let result = send_response(req, HttpResponse::ok_json(serde_json::json!({
                    "ok": true,
                    "config": config,
                    "reconnectRequired": true,
                    "address": "http://192.168.4.1/",
                    "addressNote": "After the restart the device is reachable in your home network at the IP your router assigned; the fallback AP only exists while the station link is down."
                })));
                if result.is_ok() && ota_for_network.lock().is_ok() {
                    OtaManager::schedule_restart();
                }
                result
            })
            .map_err(|error| error.to_string())?;

        server
            .fn_handler("/api/recovery", Method::Get, move |req| {
                send_response(req, HttpResponse::ok_json(crate::recovery::status()))
            })
            .map_err(|e| e.to_string())?;

        server
            .fn_handler("/api/firmware", Method::Get, move |mut req| {
                let (facts, _) = request_facts(&mut req);
                let scope = control_scope(&facts, wifi_state_firmware.station_subnet());
                let response = match crate::firmware_info::running() {
                    Ok(info) => {
                        let mut body = crate::firmware_info::firmware_info_json(&info);
                        // The setup token is what allows writes from the home
                        // network; it is disclosed only to that same scope.
                        if let Some(token) =
                            crate::net::network_api::setup_token_for(scope, setup_token_firmware.as_str())
                        {
                            if let Some(object) = body.as_object_mut() {
                                object.insert("setupToken".into(), serde_json::json!(token));
                                object.insert(
                                    "setupTokenScope".into(),
                                    serde_json::json!("station-subnet"),
                                );
                            }
                        }
                        HttpResponse::ok_json(body)
                    }
                    Err(error) => HttpResponse::err_json(500, &error),
                };
                send_response(req, response)
            })
            .map_err(|e| e.to_string())?;

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
        let wifi_state_get = Arc::clone(&wifi_state);
        server
            .fn_handler("/api/*", Method::Get, move |req| {
                if api_admission("GET", req.uri(), memory_is_low()) == ApiAdmission::Shed {
                    return send_response(
                        req,
                        HttpResponse::err_json(503, "device low on memory, shedding load"),
                    );
                }
                let uri = req.uri().to_string();
                if matches!(uri.as_str(), "/api/effects" | "/api/catalog" | "/api/state") {
                    use embedded_svc::io::Write;
                    let catalog = if uri == "/api/state" {
                        ctx.hub.snapshot_json()
                    } else {
                        ctx.hub.catalog_json()
                    };
                    // `/api/state` carries the live Wi-Fi snapshot. It is stitched
                    // in byte-wise: re-serializing the whole snapshot would need a
                    // second buffer of the same size on a memory-tight device.
                    let wifi = (uri == "/api/state")
                        .then(|| crate::net::network_api::split_state_object(&catalog))
                        .flatten()
                        .and_then(|(body, empty)| {
                            let wifi = serde_json::to_vec(&wifi_state_get.json()).ok()?;
                            Some((body.len(), empty, wifi))
                        });
                    let mut response = req.into_response(
                        200,
                        None,
                        &[
                            ("Content-Type", "application/json"),
                            ("Connection", "close"),
                        ],
                    )?;
                    if let Some((body_len, empty, wifi)) = wifi {
                        for chunk in catalog[..body_len].chunks(1024) {
                            response.write_all(chunk)?;
                        }
                        let separator: &[u8] = if empty { b"\"wifi\":" } else { b",\"wifi\":" };
                        response.write_all(separator)?;
                        for chunk in wifi.chunks(1024) {
                            response.write_all(chunk)?;
                        }
                        response.write_all(b"}")?;
                    } else {
                        for chunk in catalog.chunks(1024) {
                            response.write_all(chunk)?;
                        }
                    }
                    crate::memory_diagnostics::log_current_stack("http");
                    return Ok::<(), anyhow::Error>(());
                }
                let resp = handle_api_request(&ctx, "GET", &uri, &[]);
                crate::memory_diagnostics::log_current_stack("http");
                send_response(req, resp)
            })
            .map_err(|e| e.to_string())?;

        server
            .fn_handler("/api/recovery/retry", Method::Post, move |mut req| {
                if !trusted_request(&mut req) {
                    return send_response(
                        req,
                        HttpResponse::err_json(403, "Untrusted control request"),
                    );
                }
                match crate::recovery::retry() {
                    Ok(()) => {
                        let result = send_response(
                            req,
                            HttpResponse::ok_json(serde_json::json!({"ok":true,"restart":true})),
                        );
                        OtaManager::schedule_restart();
                        result
                    }
                    Err(error) => send_response(req, HttpResponse::err_json(503, &error)),
                }
            })
            .map_err(|e| e.to_string())?;

        // Recovery must not depend on the render actor or its command queue.
        // Register before the wildcard /api/* POST handler.
        server
            .fn_handler("/api/reboot", Method::Post, move |mut req| {
                if !trusted_request(&mut req) {
                    return send_response(
                        req,
                        HttpResponse::err_json(403, "Untrusted control request"),
                    );
                }

                let result = send_response(
                    req,
                    HttpResponse::ok_json(serde_json::json!({"ok": true, "restart": true})),
                );
                OtaManager::schedule_restart();
                result
            })
            .map_err(|e| e.to_string())?;

        // 3. Wildcard POST /api/* with bounded reads
        let ctx = api_ctx.clone();
        server
            .fn_handler("/api/*", Method::Post, move |mut req| {
                if !trusted_request(&mut req) {
                    return send_response(req, HttpResponse::err_json(403, "Untrusted control request"));
                }
                if api_admission("POST", req.uri(), memory_is_low()) == ApiAdmission::Shed {
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
                crate::memory_diagnostics::log_current_stack("http");
                send_response(req, resp)
            })
            .map_err(|e| e.to_string())?;

        // 4. Safe streaming OTA update endpoint /update
        let ota_mgr = ota_manager.clone();
        server
            .fn_handler("/update", Method::Post, move |mut req| {
                if !trusted_request(&mut req) {
                    return send_response(req, HttpResponse::err_json(403, "Untrusted firmware update request"));
                }
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
        _network_nvs: Arc<crate::nvs_store::FirmwareNvs>,
        _wifi_state: Arc<crate::net::status::WifiState>,
        _scan_cache: Arc<crate::net::scan::ScanCache>,
        _setup_token: Arc<String>,
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
