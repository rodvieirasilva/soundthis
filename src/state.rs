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
}

#[derive(Clone)]
pub struct AppState {
    enabled: Arc<AtomicBool>,
    clients: Arc<RwLock<HashMap<String, ClientInfo>>>,
    pub port: u16,
    pub lan_ip: String,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub online: bool,
    pub enabled: bool,
    pub port: u16,
    pub lan_ip: String,
    pub clients: Vec<ClientInfo>,
}

impl AppState {
    pub fn new(port: u16, lan_ip: String) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(true)),
            clients: Arc::new(RwLock::new(HashMap::new())),
            port,
            lan_ip,
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

    pub async fn remove_client(&self, id: &str) {
        let mut map = self.clients.write().await;
        map.remove(id);
    }

    pub async fn get_clients(&self) -> Vec<ClientInfo> {
        let map = self.clients.read().await;
        map.values().cloned().collect()
    }

    pub async fn to_status_response(&self) -> StatusResponse {
        StatusResponse {
            online: true,
            enabled: self.is_enabled(),
            port: self.port,
            lan_ip: self.lan_ip.clone(),
            clients: self.get_clients().await,
        }
    }
}
