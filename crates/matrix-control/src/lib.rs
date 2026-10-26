pub mod api;
pub mod config;
pub mod discovery;
#[cfg(feature = "host")]
pub mod http;
pub mod hub;
pub mod mic;
#[cfg(feature = "host")]
pub mod mqtt;
pub mod persistence;
pub mod web_ui;

pub use api::{handle_api_request, ApiContext, HttpResponse};
pub use config::{ConfigStore, PanelConfig};
pub use discovery::discovery;
#[cfg(feature = "host")]
pub use http::{start_http_server, HttpServerHandle};
pub use hub::{AudioInput, CommandMessage, ControlHub, LedOutput};
pub use mic::{MicController, MicSettings};
#[cfg(feature = "host")]
pub use mqtt::{start_mqtt_client, MqttHandle, NetworkConfig};
pub use persistence::StateStore;
pub use web_ui::WEB_UI;
