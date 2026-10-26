#[cfg(feature = "host")]
use crate::discovery::discovery;
#[cfg(feature = "host")]
use crate::hub::ControlHub;
#[cfg(feature = "host")]
use rumqttc::{Client, Event, Incoming, LastWill, MqttOptions, QoS};
#[cfg(feature = "host")]
use serde::Deserialize;
#[cfg(feature = "host")]
use std::fs;
#[cfg(feature = "host")]
use std::path::Path;
#[cfg(feature = "host")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "host")]
use std::sync::mpsc;
#[cfg(feature = "host")]
use std::sync::Arc;
#[cfg(feature = "host")]
use std::thread;
#[cfg(feature = "host")]
use std::time::{Duration, Instant};

#[cfg(feature = "host")]
#[derive(Clone, Debug, Deserialize, Default)]
pub struct NetworkConfig {
    #[serde(rename = "mqttHost")]
    pub mqtt_host: Option<String>,
    #[serde(rename = "mqttPort")]
    pub mqtt_port: Option<u16>,
    #[serde(rename = "mqttUser")]
    pub mqtt_user: Option<String>,
    #[serde(rename = "mqttPassword")]
    pub mqtt_password: Option<String>,
}

#[cfg(feature = "host")]
impl NetworkConfig {
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("failed to read network config file {}: {e}", path.display()))?;
        serde_json::from_str::<Self>(&content)
            .map_err(|e| format!("invalid network config JSON: {e}"))
    }
}

#[cfg(feature = "host")]
pub struct MqttHandle {
    shutdown: Arc<AtomicBool>,
}

#[cfg(feature = "host")]
impl MqttHandle {
    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

#[cfg(feature = "host")]
pub fn start_mqtt_client(
    hub: ControlHub,
    device_id: String,
    config: NetworkConfig,
) -> Result<Option<MqttHandle>, String> {
    let host = match config.mqtt_host {
        Some(h) if !h.trim().is_empty() => h,
        _ => return Ok(None),
    };

    let port = config.mqtt_port.unwrap_or(1883);
    let base = format!("ledmatrix/{device_id}");
    let availability_topic = format!("{base}/availability");
    let state_topic = format!("{base}/state");
    let client_id = format!("{device_id}-{}", std::process::id());

    let mut mqttoptions = MqttOptions::new(client_id, host, port);
    mqttoptions.set_keep_alive(Duration::from_secs(15));
    mqttoptions.set_clean_session(true);

    // LWT offline
    let lwt = LastWill::new(
        availability_topic.clone(),
        "offline",
        QoS::AtLeastOnce,
        true,
    );
    mqttoptions.set_last_will(lwt);

    if let (Some(u), Some(p)) = (config.mqtt_user, config.mqtt_password) {
        if !u.is_empty() {
            mqttoptions.set_credentials(u, p);
        }
    }

    // Capacity 256 for requests
    let (mut client, mut connection) = Client::new(mqttoptions, 256);
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_poller = shutdown.clone();
    let shutdown_mgr = shutdown.clone();

    let catalog = hub.catalog();
    let discovery_entries = discovery(&device_id, &catalog);

    let (event_tx, event_rx) = mpsc::sync_channel::<Event>(128);

    // Thread 1: Dedicated connection poller so client.publish never blocks or deadlocks
    thread::Builder::new()
        .name("matrix-mqtt-poller".to_string())
        .spawn(move || {
            while !shutdown_poller.load(Ordering::Relaxed) {
                match connection.recv_timeout(Duration::from_millis(50)) {
                    Ok(Ok(event)) => {
                        let _ = event_tx.send(event);
                    }
                    _ => {
                        // Timeout or connection error; connection handles reconnects
                    }
                }
            }
        })
        .map_err(|e| format!("failed to spawn MQTT poller thread: {e}"))?;

    // Thread 2: MQTT message and discovery manager
    thread::Builder::new()
        .name("matrix-mqtt-mgr".to_string())
        .spawn(move || {
            let mut last_rediscovery = Instant::now();
            let mut last_published_state = String::new();
            let mut connected = false;

            let do_publish_discovery = |client: &mut Client| {
                // 1. Availability
                let _ = client.publish(
                    &availability_topic,
                    QoS::AtLeastOnce,
                    true,
                    b"online".to_vec(),
                );

                // 2. Discovery entries
                for (topic, payload) in &discovery_entries {
                    let _ =
                        client.publish(topic, QoS::AtLeastOnce, true, payload.as_bytes().to_vec());
                }
            };

            let do_publish_state =
                |client: &mut Client, hub: &ControlHub, last_state: &mut String| {
                    let snap = hub.snapshot();
                    let snap_str = snap.to_string();
                    if snap_str != *last_state {
                        let _ = client.publish(
                            &state_topic,
                            QoS::AtLeastOnce,
                            true,
                            snap_str.as_bytes().to_vec(),
                        );
                        *last_state = snap_str;
                    }
                };

            while !shutdown_mgr.load(Ordering::Relaxed) {
                // Process incoming events
                if let Ok(event) = event_rx.recv_timeout(Duration::from_millis(50)) {
                    match event {
                        Event::Incoming(Incoming::ConnAck(_)) => {
                            connected = true;
                            // Clear last published state so fresh state is always published on ConnAck
                            last_published_state.clear();

                            // Subscribe to commands
                            let _ = client.subscribe(format!("{base}/light/set"), QoS::AtLeastOnce);
                            let _ = client.subscribe(format!("{base}/text/set"), QoS::AtLeastOnce);
                            let _ = client.subscribe(format!("{base}/speed/set"), QoS::AtLeastOnce);
                            let _ = client.subscribe(format!("{base}/cycle/set"), QoS::AtLeastOnce);
                            let _ = client
                                .subscribe(format!("{base}/audio_source/set"), QoS::AtLeastOnce);
                            let _ = client
                                .subscribe(format!("{base}/effect/+/+/set"), QoS::AtLeastOnce);
                            let _ = client.subscribe(format!("{base}/+/set"), QoS::AtLeastOnce);
                            let _ = client.subscribe("homeassistant/status", QoS::AtLeastOnce);

                            do_publish_discovery(&mut client);
                            do_publish_state(&mut client, &hub, &mut last_published_state);
                            last_rediscovery = Instant::now();
                        }
                        Event::Incoming(Incoming::Publish(publish)) => {
                            let topic = publish.topic.clone();
                            let payload = String::from_utf8_lossy(&publish.payload).to_string();

                            if topic == "homeassistant/status" {
                                if payload.trim() == "online" {
                                    do_publish_discovery(&mut client);
                                    last_published_state.clear();
                                    do_publish_state(&mut client, &hub, &mut last_published_state);
                                }
                            } else if topic == format!("{base}/light/set") {
                                let _ = hub.submit("light", &payload);
                                thread::sleep(Duration::from_millis(10));
                                do_publish_state(&mut client, &hub, &mut last_published_state);
                            } else if topic == format!("{base}/text/set") {
                                let _ = hub.submit("text", &payload);
                                thread::sleep(Duration::from_millis(10));
                                do_publish_state(&mut client, &hub, &mut last_published_state);
                            } else if topic == format!("{base}/speed/set") {
                                let _ = hub.submit("speed", &payload);
                                thread::sleep(Duration::from_millis(10));
                                do_publish_state(&mut client, &hub, &mut last_published_state);
                            } else if topic == format!("{base}/cycle/set") {
                                let _ = hub.submit("cycle", &payload);
                                thread::sleep(Duration::from_millis(10));
                                do_publish_state(&mut client, &hub, &mut last_published_state);
                            } else if topic == format!("{base}/audio_source/set") {
                                let _ = hub.submit("audio_source", &payload);
                                thread::sleep(Duration::from_millis(10));
                                do_publish_state(&mut client, &hub, &mut last_published_state);
                            } else if let Some(stripped) =
                                topic.strip_prefix(&format!("{base}/effect/"))
                            {
                                if let Some(param_path) = stripped.strip_suffix("/set") {
                                    let kind = format!("effect/{param_path}");
                                    let _ = hub.submit(&kind, &payload);
                                    thread::sleep(Duration::from_millis(10));
                                    do_publish_state(&mut client, &hub, &mut last_published_state);
                                }
                            } else if let Some(stripped) = topic.strip_prefix(&format!("{base}/")) {
                                if let Some(kind) = stripped.strip_suffix("/set") {
                                    let _ = hub.submit(kind, &payload);
                                    thread::sleep(Duration::from_millis(10));
                                    do_publish_state(&mut client, &hub, &mut last_published_state);
                                }
                            }
                        }
                        Event::Incoming(Incoming::Disconnect) => {
                            connected = false;
                        }
                        _ => {}
                    }
                }

                if connected {
                    // Periodic rediscovery every 60s
                    if last_rediscovery.elapsed() >= Duration::from_secs(60) {
                        do_publish_discovery(&mut client);
                        last_rediscovery = Instant::now();
                    }

                    // Refresh state topic when snapshot changes
                    do_publish_state(&mut client, &hub, &mut last_published_state);
                }
            }

            // Clean shutdown: publish offline before leaving
            if connected {
                let _ = client.publish(
                    &availability_topic,
                    QoS::AtLeastOnce,
                    true,
                    b"offline".to_vec(),
                );
            }
        })
        .map_err(|e| format!("failed to spawn MQTT manager thread: {e}"))?;

    Ok(Some(MqttHandle { shutdown }))
}
