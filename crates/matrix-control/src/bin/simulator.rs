use clap::Parser;
use matrix_control::config::ConfigStore;
use matrix_control::http::start_http_server;
use matrix_control::hub::{ControlHub, LedOutput};
use matrix_control::mic::MicController;
use matrix_control::mqtt::{start_mqtt_client, NetworkConfig};
use matrix_control::persistence::{FsStorageBackend, StateStore, StorageBackend};
use matrix_control::ApiContext;
use matrix_core::{Engine, Frame};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// WS2812 wire-equivalent pacing sink:
/// 1024 LEDs * 24 bits * 1.25 µs = 30.72 ms + 300 µs reset ≈ 31 ms.
struct Ws2812PacedSink {
    wire_delay: Duration,
}

impl Ws2812PacedSink {
    fn new() -> Self {
        Self {
            wire_delay: Duration::from_micros(31020),
        }
    }
}

impl LedOutput for Ws2812PacedSink {
    fn show(&mut self, _frame: &Frame, _brightness: u8) -> Result<(), String> {
        thread::sleep(self.wire_delay);
        Ok(())
    }
}

#[derive(Parser, Debug)]
#[command(name = "matrix-simulator", about = "LED Matrix Host Simulator")]
struct Args {
    #[arg(long, default_value = "127.0.0.1:8080")]
    listen: String,

    #[arg(long, default_value = "simulator")]
    device_id: String,

    #[arg(long)]
    network: Option<PathBuf>,

    #[arg(long)]
    config: Option<PathBuf>,

    #[arg(long)]
    state: Option<PathBuf>,

    #[arg(long)]
    animation: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.device_id.is_empty()
        || !args
            .device_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(
            "device-id must contain only ASCII letters, digits, underscores or hyphens".into(),
        );
    }

    // 1. Validate and load configuration
    let config_store = Arc::new(
        ConfigStore::try_new(args.config.clone())
            .map_err(|e| format!("failed to load config: {e}"))?,
    );
    let config = config_store.get();

    // 2. Validate state file if explicitly provided and exists
    if let Some(ref state_path) = args.state {
        if state_path.exists() {
            let content = FsStorageBackend::new(state_path.clone())
                .load()?
                .ok_or("state file disappeared while loading")?;
            serde_json::from_str::<serde_json::Value>(&content)
                .map_err(|e| format!("invalid state JSON in '{}': {e}", state_path.display()))?;
        }
    }

    // 3. Register built-in effects + optional LFX animation stream
    let mut effects = matrix_effects::builtin_effects();
    let maybe_anim_path: Option<PathBuf> = if let Some(ref p) = args.animation {
        Some(p.clone())
    } else if !config.fs_animation_path.is_empty() && Path::new(&config.fs_animation_path).exists()
    {
        Some(PathBuf::from(&config.fs_animation_path))
    } else {
        None
    };

    if let Some(ref anim_path) = maybe_anim_path {
        let f = std::fs::File::open(anim_path).map_err(|e| {
            format!(
                "failed to open animation file '{}': {e}",
                anim_path.display()
            )
        })?;
        let stream = matrix_core::LfxStream::new(f)
            .map_err(|e| format!("invalid LFX animation '{}': {e}", anim_path.display()))?;
        let anim_effect = matrix_effects::frame_animation(Box::new(stream), 100);

        // Replace built-in empty "Frames" effect
        if let Some(pos) = effects
            .iter()
            .position(|e| e.id().eq_ignore_ascii_case("frames"))
        {
            effects[pos] = anim_effect;
        } else {
            effects.push(anim_effect);
        }
    }

    // 4. Initialize core Engine
    let mut engine =
        Engine::new(effects).map_err(|e| format!("engine initialization failed: {e}"))?;

    // 5. Apply config defaults
    engine.apply(
        "light",
        &json!({ "brightness": config.led_brightness }).to_string(),
    )?;
    if !config.default_animation.is_empty() {
        engine.apply(
            "light",
            &json!({ "effect": config.default_animation }).to_string(),
        )?;
    }
    engine.set_auto_cycle_interval_ms(config.auto_cycle_ms);
    if config.auto_cycle_ms > 0 {
        engine.apply("cycle", "ON")?;
    }

    // 6. Restore saved runtime settings if state file exists
    let state_store = Arc::new(StateStore::new(args.state));
    state_store
        .restore(&mut engine)
        .map_err(|e| format!("failed to restore state: {e}"))?;

    // CRITICAL SAFETY INVARIANT: simulator always boots with LEDs OFF
    let _ = engine.apply("light", "{\"state\":\"OFF\"}");

    // 7. Spawn actor ControlHub
    let sink = Ws2812PacedSink::new();
    let hub =
        ControlHub::spawn(engine, sink, None).map_err(|e| format!("hub spawn failed: {e}"))?;

    let mic_controller = Arc::new(MicController::new(false));
    let api_ctx = Arc::new(ApiContext {
        hub: hub.clone(),
        config_store,
        mic_controller,
        state_store,
    });

    // 8. Start HTTP server
    let http_handle = start_http_server(&args.listen, api_ctx)
        .map_err(|e| format!("failed to start HTTP server: {e}"))?;
    println!("Matrix simulator listening on http://{}", args.listen);

    // 9. Optional MQTT client
    let mqtt_handle = if let Some(ref net_path) = args.network {
        let net_cfg = NetworkConfig::load_from_file(net_path)?;
        start_mqtt_client(hub.clone(), args.device_id.clone(), net_cfg)?
    } else {
        None
    };

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // 10. Signal handling for graceful shutdown (SIGINT / SIGTERM)
    if let Err(e) = ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }) {
        eprintln!("Warning: failed to set signal handler: {e}");
    }

    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(50));
    }

    println!("Shutting down matrix simulator...");
    if let Some(m) = mqtt_handle {
        m.stop();
    }
    http_handle.stop();
    hub.shutdown();

    Ok(())
}
