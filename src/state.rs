use crate::config::Protocol;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub connected_at: u64,
    #[serde(default = "default_listening")]
    pub listening: bool,
}

fn default_listening() -> bool {
    true
}

#[derive(Clone)]
pub struct AppState {
    enabled: Arc<AtomicBool>,
    clients: Arc<RwLock<HashMap<String, ClientInfo>>>,
    pub port: u16,
    pub lan_ip: String,
    pub protocol: Protocol,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub online: bool,
    pub enabled: bool,
    pub protocol: &'static str,
    pub port: u16,
    pub lan_ip: String,
    pub url: String,
    pub clients: Vec<ClientInfo>,
}

impl AppState {
    pub fn new(port: u16, lan_ip: String, protocol: Protocol) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(true)),
            clients: Arc::new(RwLock::new(HashMap::new())),
            port,
            lan_ip,
            protocol,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub fn set_enabled(&self, val: bool) {
        self.enabled.store(val, Ordering::SeqCst);
    }

    pub fn toggle_enabled(&self) -> bool {
        let prev = self.enabled.fetch_xor(true, Ordering::SeqCst);
        !prev
    }

    pub async fn add_client(&self, info: ClientInfo) {
        let mut map = self.clients.write().await;
        map.insert(info.id.clone(), info);
    }

    pub async fn update_client_name(&self, id: &str, name: String) {
        let mut map = self.clients.write().await;
        if let Some(client) = map.get_mut(id) {
            client.name = name;
        }
    }

    pub async fn update_client_listening(&self, id: &str, listening: bool) {
        let mut map = self.clients.write().await;
        if let Some(client) = map.get_mut(id) {
            client.listening = listening;
        }
    }

    pub async fn remove_client(&self, id: &str) {
        let mut map = self.clients.write().await;
        map.remove(id);
    }

    pub async fn get_clients(&self) -> Vec<ClientInfo> {
        let map = self.clients.read().await;
        map.values().cloned().collect()
    }

    pub async fn to_status_response(&self) -> StatusResponse {
        let scheme = self.protocol.scheme();
        let detected = crate::network::ip::get_lan_ip();
        let lan_ip = if self.lan_ip == "127.0.0.1" && detected != "127.0.0.1" {
            detected
        } else {
            self.lan_ip.clone()
        };
        let url = format!("{scheme}://{}:{}", lan_ip, self.port);
        StatusResponse {
            online: true,
            enabled: self.is_enabled(),
            protocol: scheme,
            port: self.port,
            lan_ip,
            url,
            clients: self.get_clients().await,
        }
    }
}
