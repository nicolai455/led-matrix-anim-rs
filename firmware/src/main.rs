use matrix_control::api::ApiContext;
use matrix_control::config::ConfigStore;
use matrix_control::hub::ControlHub;
use matrix_control::mic::MicController;
use matrix_control::persistence::{normalize_runtime_state, StateStore};
use matrix_core::geometry::PanelConfig;
use matrix_core::state::restore_runtime_state;
use matrix_core::Engine;
use matrix_effects::builtin_effects;
use matrix_firmware::device_id::get_device_id;
use matrix_firmware::mic::I2sMicAudioInput;
use matrix_firmware::net::{FirmwareHttpServer, FirmwareMqtt, MqttConfig};
#[cfg(target_os = "espidf")]
use matrix_firmware::net::{WifiConfig, WifiManager};
use matrix_firmware::nvs_store::FirmwareNvs;
use matrix_firmware::ota::OtaManager;
use matrix_firmware::output::{OutputSettings, RmtLedOutput};
use matrix_firmware::pins::get_default_profile;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[cfg(target_os = "espidf")]
fn mount_vfs_spiffs(base_path: &str) -> Result<(), String> {
    let base_cstr = std::ffi::CString::new(base_path).map_err(|e| e.to_string())?;
    let conf = esp_idf_sys::esp_vfs_spiffs_conf_t {
        base_path: base_cstr.as_ptr(),
        partition_label: std::ptr::null(), // automatically mounts first SPIFFS partition
        max_files: 8,
        format_if_mount_failed: false, // Never erase animation/network assets after a mount failure.
    };
    let ret = unsafe { esp_idf_sys::esp_vfs_spiffs_register(&conf) };
    if ret != esp_idf_sys::ESP_OK {
        return Err(format!("SPIFFS mount failed with code {ret}"));
    }
    log::info!("VFS SPIFFS mounted successfully at '{base_path}'");
    Ok(())
}

fn main() {
    #[cfg(target_os = "espidf")]
    {
        esp_idf_svc::sys::link_patches();
        let _ = esp_idf_svc::log::EspLogger::initialize_default();

        let conf = esp_idf_hal::task::thread::ThreadSpawnConfiguration {
            name: Some(b"firmware-main\0"),
            stack_size: 24576,
            priority: 5,
            inherit: false,
            ..Default::default()
        };
        conf.set()
            .expect("failed to configure firmware-main thread");
    }

    let thread = std::thread::Builder::new()
        .name("firmware-main".to_string())
        .stack_size(24576)
        .spawn(run_firmware)
        .expect("failed to spawn firmware-main thread");

    let _ = thread.join();
}

#[inline(never)]
fn load_panel_config(
    nvs: &FirmwareNvs,
    profile: &matrix_firmware::pins::BoardProfile,
) -> PanelConfig {
    let board_defaults = PanelConfig {
        data_pin: profile.data_pin,
        panel_pins: profile.panel_pins,
        ..PanelConfig::default()
    };
    let file_config = std::fs::File::open("/spiffs/config/panel_config.json")
        .ok()
        .and_then(|file| {
            use std::io::Read;
            let mut body = String::new();
            file.take(8193).read_to_string(&mut body).ok()?;
            if body.len() > 8192 {
                return None;
            }
            let value: serde_json::Value = serde_json::from_str(&body).ok()?;
            let mut base = serde_json::to_value(&board_defaults).ok()?;
            for (key, value) in value.as_object()? {
                base.as_object_mut()?.insert(key.clone(), value.clone());
            }
            serde_json::from_value::<PanelConfig>(base)
                .ok()
                .filter(|cfg| cfg.validate().is_ok())
        });
    nvs.load_panel_config()
        .filter(|config| config.validate().is_ok())
        .or(file_config)
        .unwrap_or(board_defaults)
}

#[inline(never)]
fn run_firmware() {
    #[cfg(target_os = "espidf")]
    unsafe {
        let stack_watermark = esp_idf_sys::uxTaskGetStackHighWaterMark(std::ptr::null_mut());
        let free_heap = esp_idf_sys::esp_get_free_heap_size();
        let min_free_heap = esp_idf_sys::esp_get_minimum_free_heap_size();
        log::info!("firmware-main started! Stack high-water mark: {stack_watermark}, free heap: {free_heap}, min free: {min_free_heap}");
    }

    let profile = get_default_profile();
    log::info!("=== Matrix Firmware booting on {} ===", profile.name);

    // 1. Mount SPIFFS storage filesystem
    #[cfg(target_os = "espidf")]
    if let Err(e) = mount_vfs_spiffs("/spiffs") {
        log::warn!("Proceeding without SPIFFS VFS: {e}");
    }

    // 2. Initialize real NVS storage backend
    log::info!("Boot step 2: taking default NVS partition...");
    #[cfg(target_os = "espidf")]
    let default_nvs = match esp_idf_svc::nvs::EspDefaultNvsPartition::take() {
        Ok(n) => n,
        Err(e) => {
            log::error!("Failed to take default NVS partition: {e}");
            panic!("NVS initialization failure");
        }
    };

    log::info!("Boot step 2b: creating FirmwareNvs...");
    #[cfg(target_os = "espidf")]
    let nvs = match FirmwareNvs::new(default_nvs.clone()) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to initialize NVS store: {e}");
            panic!("NVS initialization failure");
        }
    };
    #[cfg(not(target_os = "espidf"))]
    let nvs = match FirmwareNvs::new() {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to initialize NVS store: {e}");
            panic!("NVS initialization failure");
        }
    };

    // 3. Load or initialize panel configuration
    log::info!("Boot step 3: loading panel config...");
    let panel_config = load_panel_config(&nvs, &profile);
    log::info!("Boot step 4: registering builtin effects...");

    // 4. Register built-in effect catalog + optional streaming LFX asset
    let mut effects = builtin_effects();
    log::info!("Boot step 4b: checking LFX asset...");
    let asset = Path::new(panel_config.fs_animation_path.trim_start_matches('/'));
    if asset
        .components()
        .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        panic!("Animation path cannot escape asset directory");
    }
    let lfx_path = Path::new("/spiffs").join(asset);
    log::info!("Opening LFX asset at {}", lfx_path.display());
    let file_res = std::fs::File::open(&lfx_path);
    log::info!("File::open result: is_ok={}", file_res.is_ok());
    if let Ok(f) = file_res {
        log::info!("Calling LfxStream::new...");
        match matrix_core::LfxStream::new(f) {
            Ok(stream) => {
                log::info!("Registered streaming LFX asset from {}", lfx_path.display());
                if let Some(slot) = effects.iter_mut().find(|effect| effect.id() == "Frames") {
                    log::info!("Replacing Frames slot with frame_animation...");
                    *slot = matrix_effects::frame_animation(Box::new(stream), 100);
                    log::info!("Replaced Frames slot!");
                }
            }
            Err(e) => log::warn!("Failed to parse LFX stream: {e}"),
        }
    } else {
        log::info!("No LFX asset found at {}", lfx_path.display());
    }

    // 5. Initialize Engine
    log::info!("Boot step 5: initializing Engine...");
    let mut engine = match Engine::new(effects) {
        Ok(eng) => eng,
        Err(e) => {
            log::error!("Failed to create core Engine: {e}");
            panic!("Engine creation failure");
        }
    };

    if !engine.switch_to_effect(&panel_config.default_animation) {
        log::warn!("Unknown configured default effect; retaining initial effect");
    }
    engine
        .apply(
            "light",
            &serde_json::json!({"brightness":panel_config.led_brightness}).to_string(),
        )
        .expect("validated brightness");
    engine.set_auto_cycle_interval_ms(panel_config.auto_cycle_ms);
    if panel_config.auto_cycle_ms != 0 {
        engine.apply("cycle", "ON").expect("valid cycle command");
    }

    // 6. Restore persistent runtime state from NVS if present
    if let Some(state_val) = nvs.load_saved_state() {
        let normalized = normalize_runtime_state(&state_val).to_string();
        if let Err(e) = restore_runtime_state(&mut engine, &normalized) {
            log::warn!("Runtime state restore non-fatal error: {e}");
        } else {
            log::info!("Restored persistent state successfully");
        }
    }

    // CRITICAL HARDWARE & SAFETY INVARIANT: Boots OFF including restore!
    // WS2812B LEDs always start dark on power up / reset.
    let _ = engine.apply("light", "{\"state\":\"OFF\"}");

    // 7. Initialize LED Output
    let output_settings = OutputSettings::from_panel_config(&panel_config);
    let mut rmt_output = RmtLedOutput::new(output_settings, panel_config.clone());
    if let Err(error) = rmt_output.blank() {
        panic!("Cannot establish safe dark output: {error}");
    }

    // 8. Initialize Microphone
    let has_mic = profile.mic_pins.is_some();
    let initial_mic = nvs
        .load_mic_settings()
        .unwrap_or_else(|| engine.mic_settings().clone());
    let mic_input = I2sMicAudioInput::new(has_mic, initial_mic);
    let mic_quiesce = mic_input.quiesce_handle();
    let mic_controller = Arc::new(MicController::new(has_mic));

    // 9. Wire shared ApiContext storage using real NVS backend hooks
    let config_store = Arc::new(ConfigStore::with_backend(
        nvs.config_backend(),
        Some(panel_config.clone()),
    ));
    let state_store = Arc::new(StateStore::with_backend(nvs.state_backend()));

    // 10. Spawn ControlHub render actor thread
    let output_quiesce_flag = rmt_output.get_quiesce_handle();
    let audio_input: Option<Box<dyn matrix_control::AudioInput>> = if has_mic {
        Some(Box::new(mic_input))
    } else {
        None
    };
    let hub = match ControlHub::spawn(engine, rmt_output, audio_input) {
        Ok(h) => h,
        Err(e) => {
            log::error!("Failed to spawn ControlHub render actor: {e}");
            panic!("ControlHub spawn failure");
        }
    };

    let api_ctx = Arc::new(ApiContext {
        hub: hub.clone(),
        config_store: Arc::clone(&config_store),
        mic_controller: Arc::clone(&mic_controller),
        state_store: Arc::clone(&state_store),
    });

    // 11. OTA Manager with verified hardware quiesce handshake
    let ota_manager = Arc::new(Mutex::new(OtaManager::new()));
    {
        let output_for_pause = output_quiesce_flag.clone();
        let output_for_resume = output_quiesce_flag.clone();
        if let Ok(mut ota) = ota_manager.lock() {
            let mic_for_pause = mic_quiesce.clone();
            let mic_for_resume = mic_quiesce.clone();
            ota.set_quiesce_hooks(
                move || {
                    mic_for_pause.pause()?;
                    if let Err(error) = output_for_pause.pause(Duration::from_secs(1)) {
                        mic_for_pause.resume();
                        return Err(error);
                    }
                    Ok(())
                },
                move || {
                    output_for_resume.resume();
                    mic_for_resume.resume();
                },
            );
        }
    }

    // 12. Networking (Wi-Fi, HTTP, MQTT)
    let device_id = get_device_id();
    log::info!("Device ID: {device_id}");

    #[cfg(target_os = "espidf")]
    let (modem, sysloop) = match (
        esp_idf_hal::peripherals::Peripherals::take().map(|p| p.modem),
        esp_idf_svc::eventloop::EspSystemEventLoop::take(),
    ) {
        (Ok(m), Ok(s)) => (m, s),
        _ => {
            log::warn!("Could not take modem or system event loop; networking skipped");
            loop {
                std::thread::sleep(Duration::from_secs(60));
            }
        }
    };

    let network = match matrix_firmware::network_config::NetworkConfig::load(Path::new(
        "/spiffs/config/network.json",
    )) {
        Ok(config) => config,
        Err(error) => {
            log::error!("Networking disabled: {error}");
            None
        }
    };
    #[cfg(target_os = "espidf")]
    let _wifi = match WifiManager::start(
        modem,
        sysloop,
        default_nvs,
        network
            .as_ref()
            .filter(|c| !c.wifi_ssid.is_empty())
            .map(|c| WifiConfig {
                ssid: c.wifi_ssid.clone(),
                password: (!c.wifi_password.is_empty()).then(|| c.wifi_password.clone()),
            }),
    ) {
        Ok(w) => {
            log::info!("Wi-Fi service initialized");
            Some(w)
        }
        Err(e) => {
            log::warn!("Wi-Fi initialization error: {e}");
            None
        }
    };

    let _http_server = match FirmwareHttpServer::start(api_ctx, Arc::clone(&ota_manager)) {
        Ok(s) => {
            log::info!("HTTP server operational on port 80");
            Some(s)
        }
        Err(e) => {
            log::error!("HTTP server startup error: {e}");
            None
        }
    };

    let _mqtt = network
        .filter(|c| !c.mqtt_host.is_empty())
        .and_then(|config| {
            let mqtt_config = MqttConfig {
                host: config.mqtt_host,
                port: config.mqtt_port,
                username: (!config.mqtt_user.is_empty()).then_some(config.mqtt_user),
                password: (!config.mqtt_password.is_empty()).then_some(config.mqtt_password),
            };
            match FirmwareMqtt::start(device_id, hub.clone(), mqtt_config) {
                Ok(client) => Some(client),
                Err(error) => {
                    log::error!("MQTT startup failed: {error}");
                    None
                }
            }
        });

    // Retain lifetimes forever: prevent main thread from exiting and dropping resources
    log::info!("Firmware main bootstrap complete. Entering supervisor loop.");
    loop {
        #[cfg(target_os = "espidf")]
        unsafe {
            log::info!(
                "Runtime memory: free heap={}, minimum free heap={}, supervisor stack watermark={}",
                esp_idf_sys::esp_get_free_heap_size(),
                esp_idf_sys::esp_get_minimum_free_heap_size(),
                esp_idf_sys::uxTaskGetStackHighWaterMark(std::ptr::null_mut())
            );
            let mut access_point: esp_idf_sys::wifi_ap_record_t = std::mem::zeroed();
            if esp_idf_sys::esp_wifi_sta_get_ap_info(&mut access_point) == esp_idf_sys::ESP_OK {
                let _ = hub.submit("rssi", &access_point.rssi.to_string());
            }
        }
        std::thread::sleep(Duration::from_secs(10));
    }
}
