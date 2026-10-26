use crate::config::ConfigStore;
use crate::hub::{ControlHub, SubmitOutcome};
use crate::mic::MicController;
use crate::persistence::StateStore;
use crate::web_ui::WEB_UI;
use matrix_core::audio::MicSettings;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

// A static reference gives both host/API and firmware streaming paths one asset.
// A const byte slice can be materialized twice across the crate boundary.
pub static PREVIEW_WASM: &[u8] = include_bytes!("../assets/preview.wasm.gz");

pub struct ApiContext {
    pub hub: ControlHub,
    pub config_store: Arc<ConfigStore>,
    pub mic_controller: Arc<MicController>,
    pub state_store: Arc<StateStore>,
}

pub struct HttpResponse {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn ok_html(html: &'static str) -> Self {
        Self {
            status: 200,
            content_type: "text/html; charset=utf-8",
            body: html.as_bytes().to_vec(),
        }
    }

    pub fn ok_json(val: Value) -> Self {
        Self {
            status: 200,
            content_type: "application/json",
            body: serde_json::to_vec(&val).unwrap_or_default(),
        }
    }

    pub fn accepted_json(val: Value) -> Self {
        Self {
            status: 202,
            content_type: "application/json",
            body: serde_json::to_vec(&val).unwrap_or_default(),
        }
    }

    pub fn ok_binary(content_type: &'static str, bytes: Vec<u8>) -> Self {
        Self {
            status: 200,
            content_type,
            body: bytes,
        }
    }

    pub fn err_json(status: u16, msg: &str) -> Self {
        Self {
            status,
            content_type: "application/json",
            body: serde_json::to_vec(&json!({ "error": msg })).unwrap_or_default(),
        }
    }
}

fn map_submit_result(res: Result<SubmitOutcome, String>) -> HttpResponse {
    match res {
        Ok(SubmitOutcome::Applied) => HttpResponse::ok_json(json!({ "ok": true })),
        Ok(SubmitOutcome::Pending) => {
            HttpResponse::accepted_json(json!({ "ok": true, "pending": true }))
        }
        Err(e) if e.contains("busy") => HttpResponse::err_json(503, &e),
        Err(e) => HttpResponse::err_json(400, &e),
    }
}

pub fn handle_api_request(ctx: &ApiContext, method: &str, path: &str, body: &[u8]) -> HttpResponse {
    let method = method.to_uppercase();

    match (method.as_str(), path) {
        ("GET", "/") => HttpResponse::ok_html(WEB_UI),
        ("GET", "/preview.wasm.gz") => {
            HttpResponse::ok_binary("application/gzip", PREVIEW_WASM.to_vec())
        }
        ("GET", "/api/audio") => {
            let snapshot = ctx.hub.snapshot();
            let mut audio = serde_json::Map::new();
            for key in [
                "bands",
                "level",
                "peak",
                "beat",
                "waveform",
                "audio_status",
                "renderTimeMs",
            ] {
                if let Some(value) = snapshot.get(key) {
                    audio.insert(key.into(), value.clone());
                }
            }
            HttpResponse::ok_json(Value::Object(audio))
        }

        ("GET", "/api/state") => HttpResponse::ok_json(ctx.hub.snapshot()),

        ("GET", "/api/effects") | ("GET", "/api/catalog") => HttpResponse {
            status: 200,
            content_type: "application/json",
            body: ctx.hub.catalog_json().as_ref().clone(),
        },

        ("GET", "/api/frame") => {
            let frame = ctx.hub.frame();
            let mut bytes = Vec::with_capacity(3072);
            for p in &frame.pixels {
                bytes.push(p.r);
                bytes.push(p.g);
                bytes.push(p.b);
            }
            HttpResponse::ok_binary("application/octet-stream", bytes)
        }

        ("GET", "/api/status") => {
            let snap = ctx.hub.snapshot();
            let fallback = Value::Null;
            HttpResponse::ok_json(json!({
                "fps": ctx.hub.fps(),
                "state": snap.get("state").unwrap_or(&fallback),
                "heap": 128000,
                "uptime": ctx.hub.uptime().as_secs(),
                "outputErrors": ctx.hub.output_error_count(),
                "lastError": ctx.hub.last_output_error(),
                "hasMic": ctx.hub.has_mic_hardware(),
            }))
        }

        ("GET", "/api/config") => HttpResponse::ok_json(ctx.config_store.to_json()),

        ("POST", "/api/config") => {
            let val = match serde_json::from_slice::<Value>(body) {
                Ok(v) => v,
                Err(e) => return HttpResponse::err_json(400, &format!("invalid JSON: {e}")),
            };
            let old_cfg = ctx.config_store.get();
            match ctx.config_store.update_from_json(&val) {
                Ok(updated) => {
                    let restart_required = old_cfg.matrix_width != updated.matrix_width
                        || old_cfg.matrix_height != updated.matrix_height
                        || old_cfg.panel_order != updated.panel_order
                        || old_cfg.panel_rotation != updated.panel_rotation
                        || old_cfg.serpentine != updated.serpentine
                        || old_cfg.start_corner != updated.start_corner
                        || old_cfg.panel_layout != updated.panel_layout
                        || old_cfg.panel_serpentine != updated.panel_serpentine
                        || old_cfg.mirror_x != updated.mirror_x
                        || old_cfg.data_pin != updated.data_pin
                        || old_cfg.parallel_output != updated.parallel_output
                        || old_cfg.panel_pins != updated.panel_pins;

                    // Live power budget update to render actor
                    let _ = ctx.hub.submit_sync(
                        "power_limit",
                        &updated.led_max_milliamps.to_string(),
                        Duration::from_millis(50),
                    );

                    HttpResponse::ok_json(json!({
                        "ok": true,
                        "config": updated,
                        "restart": if restart_required { "required" } else { "none" },
                        "restartRequired": restart_required
                    }))
                }
                Err(e) => HttpResponse::err_json(400, &e),
            }
        }

        ("GET", "/api/mic") => {
            let snap = ctx.hub.snapshot();
            let active = snap.get("audio_source").and_then(|v| v.as_str()) == Some("Mic");
            let settings = snap
                .get("mic")
                .and_then(|s| serde_json::from_value::<MicSettings>(s.clone()).ok())
                .unwrap_or_default();
            if let Some((raw, gain)) = ctx.hub.live_metrics() {
                ctx.mic_controller.set_metrics(raw, gain);
            }
            HttpResponse::ok_json(ctx.mic_controller.to_json(active, &settings, Some(&snap)))
        }

        ("POST", "/api/mic") => {
            let body_str = match std::str::from_utf8(body) {
                Ok(s) => s,
                Err(_) => return HttpResponse::err_json(400, "invalid UTF-8 body"),
            };
            map_submit_result(
                ctx.hub
                    .submit_sync("mic", body_str, Duration::from_millis(50)),
            )
        }

        ("POST", "/api/state/save") => {
            let snap = ctx.hub.snapshot();
            match ctx.state_store.save(&snap) {
                Ok(_) => HttpResponse::ok_json(json!({ "ok": true })),
                Err(e) => HttpResponse::err_json(500, &e),
            }
        }

        ("POST", "/api/state/clear") => match ctx.state_store.clear() {
            Ok(_) => HttpResponse::ok_json(json!({ "ok": true })),
            Err(e) => HttpResponse::err_json(500, &e),
        },

        ("POST", "/api/command") => {
            let parsed = match serde_json::from_slice::<Value>(body) {
                Ok(v) => v,
                Err(e) => return HttpResponse::err_json(400, &format!("invalid JSON: {e}")),
            };

            let kind = match parsed.get("kind").and_then(|v| v.as_str()) {
                Some(k) => k,
                None => return HttpResponse::err_json(400, "missing kind"),
            };

            let payload_str = match parsed.get("payload") {
                Some(Value::String(s)) => s.clone(),
                Some(val @ Value::Object(_)) => val.to_string(),
                Some(val @ Value::Number(_)) => val.to_string(),
                Some(val @ Value::Bool(_)) => val.to_string(),
                Some(val) => val.to_string(),
                None => "".to_string(),
            };

            map_submit_result(
                ctx.hub
                    .submit_sync(kind, &payload_str, Duration::from_millis(50)),
            )
        }

        // Legacy compatibility routes
        ("POST", "/api/power") => {
            let parsed = match serde_json::from_slice::<Value>(body) {
                Ok(v) => v,
                Err(e) => return HttpResponse::err_json(400, &format!("invalid JSON: {e}")),
            };
            let state = parsed.get("state").and_then(|v| v.as_str()).unwrap_or("");
            if state != "ON" && state != "OFF" {
                return HttpResponse::err_json(400, "invalid state");
            }
            let payload = json!({ "state": state }).to_string();
            map_submit_result(
                ctx.hub
                    .submit_sync("light", &payload, Duration::from_millis(50)),
            )
        }

        ("POST", "/api/brightness") => {
            let parsed = match serde_json::from_slice::<Value>(body) {
                Ok(v) => v,
                Err(e) => return HttpResponse::err_json(400, &format!("invalid JSON: {e}")),
            };
            let val = match parsed.get("value").and_then(|v| v.as_i64()) {
                Some(v) if (0..=255).contains(&v) => v,
                _ => return HttpResponse::err_json(400, "invalid value"),
            };
            let payload = json!({ "brightness": val }).to_string();
            map_submit_result(
                ctx.hub
                    .submit_sync("light", &payload, Duration::from_millis(50)),
            )
        }

        ("POST", "/api/effect") => {
            let parsed = match serde_json::from_slice::<Value>(body) {
                Ok(v) => v,
                Err(e) => return HttpResponse::err_json(400, &format!("invalid JSON: {e}")),
            };
            let name = match parsed.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => return HttpResponse::err_json(400, "missing effect name"),
            };
            let payload = json!({ "effect": name }).to_string();
            map_submit_result(
                ctx.hub
                    .submit_sync("light", &payload, Duration::from_millis(50)),
            )
        }

        ("POST", "/api/effect/param") => {
            let parsed = match serde_json::from_slice::<Value>(body) {
                Ok(v) => v,
                Err(e) => return HttpResponse::err_json(400, &format!("invalid JSON: {e}")),
            };
            let effect = match parsed.get("effect").and_then(|v| v.as_str()) {
                Some(e) => e,
                None => return HttpResponse::err_json(400, "missing effect"),
            };
            let param = match parsed.get("param").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return HttpResponse::err_json(400, "missing param"),
            };
            let value_str = match parsed.get("value") {
                Some(Value::Number(n)) => n.to_string(),
                Some(Value::String(s)) => s.clone(),
                Some(Value::Bool(b)) => if *b { "1" } else { "0" }.to_string(),
                _ => return HttpResponse::err_json(400, "missing or invalid value"),
            };
            let kind = format!("effect/{effect}/{param}");
            map_submit_result(
                ctx.hub
                    .submit_sync(&kind, &value_str, Duration::from_millis(50)),
            )
        }

        ("POST", "/api/text") => {
            let parsed = match serde_json::from_slice::<Value>(body) {
                Ok(v) => v,
                Err(e) => return HttpResponse::err_json(400, &format!("invalid JSON: {e}")),
            };
            let text = parsed.get("text").and_then(|v| v.as_str()).unwrap_or("");
            map_submit_result(ctx.hub.submit_sync("text", text, Duration::from_millis(50)))
        }

        ("POST", "/api/speed") => {
            let parsed = match serde_json::from_slice::<Value>(body) {
                Ok(v) => v,
                Err(e) => return HttpResponse::err_json(400, &format!("invalid JSON: {e}")),
            };
            let speed = match parsed.get("value").and_then(|v| v.as_i64()) {
                Some(s) if (0..=120).contains(&s) => s,
                _ => return HttpResponse::err_json(400, "invalid speed"),
            };
            map_submit_result(ctx.hub.submit_sync(
                "speed",
                &speed.to_string(),
                Duration::from_millis(50),
            ))
        }

        ("POST", "/api/cycle") => {
            let parsed = match serde_json::from_slice::<Value>(body) {
                Ok(v) => v,
                Err(e) => return HttpResponse::err_json(400, &format!("invalid JSON: {e}")),
            };
            let state = parsed.get("state").and_then(|v| v.as_str()).unwrap_or("");
            if state != "ON" && state != "OFF" {
                return HttpResponse::err_json(400, "invalid cycle state");
            }
            map_submit_result(
                ctx.hub
                    .submit_sync("cycle", state, Duration::from_millis(50)),
            )
        }

        ("POST", "/api/audio") => {
            let parsed = match serde_json::from_slice::<Value>(body) {
                Ok(v) => v,
                Err(e) => return HttpResponse::err_json(400, &format!("invalid JSON: {e}")),
            };
            let source = match parsed.get("source").and_then(|v| v.as_str()) {
                Some(s) if s == "Off" || s == "Synthetic" || s == "Mic" => s,
                _ => return HttpResponse::err_json(400, "invalid audio source"),
            };
            map_submit_result(ctx.hub.submit_sync(
                "audio_source",
                source,
                Duration::from_millis(50),
            ))
        }

        _ => HttpResponse::err_json(404, "not found"),
    }
}
