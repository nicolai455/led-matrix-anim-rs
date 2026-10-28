//! Firmware MQTT transport. Callback only copies bounded events; the worker
//! owns publication/subscription and never executes in the MQTT callback task.
use matrix_control::ControlHub;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const MAX_MQTT_PAYLOAD_BYTES: usize = 4096;
#[derive(Clone)]
pub struct MqttConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}
pub struct FirmwareMqtt {
    shutdown: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
#[cfg(target_os = "espidf")]
enum Inbound {
    Data {
        topic: Option<String>,
        bytes: Vec<u8>,
        total: usize,
        offset: usize,
    },
}
impl FirmwareMqtt {
    #[cfg(target_os = "espidf")]
    pub fn start(device_id: String, hub: ControlHub, config: MqttConfig) -> Result<Self, String> {
        use embedded_svc::mqtt::client::Details;
        use esp_idf_svc::mqtt::client::{
            EspMqttClient, EventPayload, LwtConfiguration, MqttClientConfiguration, QoS,
        };
        use std::sync::mpsc::{sync_channel, RecvTimeoutError};
        use std::time::{Duration, Instant};
        let base = format!("ledmatrix/{device_id}");
        let availability = format!("{base}/availability");
        let uri = format!("mqtt://{}:{}", config.host, config.port);
        let options = MqttClientConfiguration {
            client_id: Some(&device_id),
            username: config.username.as_deref(),
            password: config.password.as_deref(),
            lwt: Some(LwtConfiguration {
                topic: &availability,
                payload: b"offline",
                qos: QoS::AtLeastOnce,
                retain: true,
            }),
            ..Default::default()
        };
        let (tx, rx) = sync_channel(16);
        let connection = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let dropped = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let callback_connection = connection.clone();
        let callback_dropped = dropped.clone();
        let mut client = EspMqttClient::new_cb(&uri, &options, move |event| {
            let message = match event.payload() {
                EventPayload::Connected(_) => {
                    callback_connection.fetch_add(2, Ordering::AcqRel);
                    callback_connection.fetch_or(1, Ordering::Release);
                    None
                }
                EventPayload::Disconnected => {
                    callback_connection.fetch_and(!1, Ordering::Release);
                    None
                }
                EventPayload::Received {
                    topic,
                    data,
                    details,
                    ..
                } => {
                    let (total, offset) = match details {
                        Details::Complete => (data.len(), 0),
                        Details::InitialChunk(d) => (d.total_data_size, 0),
                        Details::SubsequentChunk(d) => (d.total_data_size, d.current_data_offset),
                    };
                    if total > MAX_MQTT_PAYLOAD_BYTES
                        || data.len() > total
                        || topic.is_some_and(|t| t.len() > 256)
                    {
                        None
                    } else {
                        Some(Inbound::Data {
                            topic: topic.map(str::to_owned),
                            bytes: data.to_vec(),
                            total,
                            offset,
                        })
                    }
                }
                _ => None,
            };
            if let Some(message) = message {
                if tx.try_send(message).is_err() {
                    callback_dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
        })
        .map_err(|e| format!("MQTT initialization failed: {e}"))?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let stop = shutdown.clone();
        let worker = std::thread::Builder::new()
            .name("mqtt-worker".into())
            .stack_size(12288)
            .spawn(move || {
                let mut connected = false;
                let mut refresh = true;
                let mut last_discovery = Instant::now();
                let mut last_state: Option<Arc<Vec<u8>>> = None;
                let mut topic = String::new();
                let mut body = Vec::new();
                let mut connection_seen=0;
                while !stop.load(Ordering::Relaxed) {
                    let state=connection.load(Ordering::Acquire);
                    if state!=connection_seen {
                        connection_seen=state;
                        connected=state&1 != 0;
                        body.clear(); topic.clear(); last_state = None;
                        refresh=true;
                        if connected {
                            for subscription in [format!("{base}/+/set"),format!("{base}/effect/+/+/set"),"homeassistant/status".into()] {
                                if let Err(error)=client.subscribe(&subscription,QoS::AtLeastOnce) {log::warn!("MQTT subscribe failed: {error}");}
                            }
                        }
                    }
                    let losses=dropped.swap(0,Ordering::Relaxed);
                    if losses>0 {
                        body.clear();topic.clear();refresh=true;
                        log::warn!("MQTT inbound queue dropped {losses} chunks; discarding partial command");
                    }
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(Inbound::Data {
                            topic: incoming,
                            bytes,
                            total,
                            offset,
                        }) => {
                            if offset == 0 {
                                body.clear();
                                topic = incoming.unwrap_or_default();
                            }
                            if offset != body.len()
                                || total > MAX_MQTT_PAYLOAD_BYTES
                                || offset + bytes.len() > total
                            {
                                body.clear();
                                topic.clear();
                                continue;
                            }
                            body.extend_from_slice(&bytes);
                            if body.len() == total {
                                if let Ok(payload) = std::str::from_utf8(&body) {
                                    if topic == "homeassistant/status" && payload == "online" {
                                        refresh = true;
                                        last_state = None;
                                    } else {
                                        handle_mqtt_message(&hub, &base, &topic, payload);
                                    }
                                }
                                body.clear();
                                topic.clear();
                            }
                        }
                        Err(RecvTimeoutError::Disconnected) => break,
                        Err(RecvTimeoutError::Timeout) => {}
                    }
                    if !connected {
                        continue;
                    }
                    if refresh || last_discovery.elapsed() >= Duration::from_secs(60) {
                        let catalog = hub.catalog_json();
                        let announced = client
                            .publish(&availability, QoS::AtLeastOnce, true, b"online")
                            .is_ok();
                        let published = matrix_control::discovery::discovery_json_for_each(
                            &device_id,
                            catalog.as_slice(),
                            |(topic, payload)| client
                                .publish(&topic, QoS::AtLeastOnce, true, payload.as_bytes())
                                .map(|_| ()),
                        ).is_ok();
                        refresh = !(announced && published);
                        last_discovery = Instant::now();
                    }
                    let state = hub.snapshot_json();
                    if last_state.as_ref().is_none_or(|last| state.as_slice() != last.as_slice())
                        && client
                            .publish(
                                &format!("{base}/state"),
                                QoS::AtLeastOnce,
                                true,
                                state.as_slice(),
                            )
                            .is_ok()
                    {
                        last_state = Some(state);
                    }
                }
                let _ = client.publish(&availability, QoS::AtLeastOnce, true, b"offline");
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            shutdown,
            worker: Some(worker),
        })
    }
    #[cfg(not(target_os = "espidf"))]
    pub fn start(
        _device_id: String,
        _hub: ControlHub,
        _config: MqttConfig,
    ) -> Result<Self, String> {
        Err("ESP MQTT transport unavailable on host".into())
    }
}
impl Drop for FirmwareMqtt {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
pub fn handle_mqtt_message(hub: &ControlHub, base: &str, topic: &str, payload: &str) {
    if payload.len() > MAX_MQTT_PAYLOAD_BYTES {
        return;
    }
    if let Some(kind) = topic
        .strip_prefix(&format!("{base}/"))
        .and_then(|s| s.strip_suffix("/set"))
    {
        if matches!(kind, "light" | "text" | "cycle") {
            // Payloads can contain private text; log only the command category.
            log::info!("MQTT control command received: {kind}");
        }
        if let Err(error) = hub.submit(kind, payload) {
            log::warn!("MQTT command rejected: {error}");
        }
    }
}
