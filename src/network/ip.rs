use std::net::UdpSocket;

/// Discovers the primary outbound LAN IP of the current machine.
pub fn get_lan_ip() -> String {
    // Attempt connecting a UDP socket to an external address (no traffic is sent)
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(local_addr) = socket.local_addr() {
                let ip = local_addr.ip();
                if !ip.is_loopback() {
                    return ip.to_string();
                }
            }
        }
    }

    "127.0.0.1".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lan_ip_not_empty() {
        let ip = get_lan_ip();
        assert!(!ip.is_empty());
    }
}
