//! mDNS service discovery using the `mdns-sd` crate.
//!
//! Registers a `_linux-screenextend._tcp.local.` service on the local network
//! and discovers other instances of the same service.

use anyhow::{Context, Result};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use screenextend_common::protocol::MDNS_SERVICE_TYPE;
use screenextend_common::types::PeerInfo;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Events emitted by the discovery service.
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    /// A new peer was discovered on the network.
    PeerDiscovered(PeerInfo),
    /// A previously discovered peer has gone offline.
    PeerLost(String),
}

/// mDNS-based service discovery for finding peers on the local network.
pub struct DiscoveryService {
    daemon: ServiceDaemon,
    /// Currently known peers, keyed by their full service name.
    peers: Arc<Mutex<HashMap<String, PeerInfo>>>,
    /// Channel to send discovery events to the application.
    event_tx: mpsc::Sender<DiscoveryEvent>,
    /// Instance name for this service.
    instance_name: String,
    /// Whether the service is currently registered.
    registered: bool,
}

impl DiscoveryService {
    /// Create a new discovery service.
    ///
    /// # Arguments
    /// * `hostname` - Human-readable name for this machine
    /// * `_port` - The signaling port this instance listens on
    /// * `event_tx` - Channel to receive discovery events
    pub fn new(
        hostname: &str,
        _port: u16,
        event_tx: mpsc::Sender<DiscoveryEvent>,
    ) -> Result<Self> {
        let daemon = ServiceDaemon::new()
            .context("Failed to create mDNS daemon")?;

        let instance_name = format!("screenextend-{}", hostname);

        Ok(Self {
            daemon,
            peers: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
            instance_name,
            registered: false,
        })
    }

    /// Register this instance as a service on the network.
    pub fn register(&mut self, port: u16, is_host: bool) -> Result<()> {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".into());

        let mut properties = HashMap::new();
        properties.insert("role".to_string(), if is_host { "host" } else { "client" }.to_string());
        properties.insert("hostname".to_string(), hostname.clone());
        properties.insert("version".to_string(), "1".to_string());

        let host_name = if hostname.ends_with(".local.") {
            hostname.clone()
        } else if let Some(stripped) = hostname.strip_suffix(".local") {
            format!("{}.local.", stripped)
        } else if let Some(stripped) = hostname.strip_suffix('.') {
            format!("{}.local.", stripped)
        } else {
            format!("{}.local.", hostname)
        };

        let service_info = ServiceInfo::new(
            MDNS_SERVICE_TYPE,
            &self.instance_name,
            &host_name,
            "",  // Let mdns-sd figure out the IP
            port,
            properties,
        )
        .with_context(|| format!("Failed to create ServiceInfo (type={}, instance={}, host={})", MDNS_SERVICE_TYPE, self.instance_name, host_name))?;

        self.daemon
            .register(service_info)
            .map_err(|e| anyhow::anyhow!("Failed to register mDNS service: {:?}", e))?;

        self.registered = true;
        info!("Registered mDNS service: {} on port {}", self.instance_name, port);
        Ok(())
    }

    /// Start browsing for peers on the network.
    ///
    /// This spawns a background tokio task that processes mDNS events
    /// and forwards them as `DiscoveryEvent`s.
    pub fn start_browsing(&self) -> Result<()> {
        let receiver = self
            .daemon
            .browse(MDNS_SERVICE_TYPE)
            .context("Failed to start mDNS browse")?;

        let peers = Arc::clone(&self.peers);
        let event_tx = self.event_tx.clone();
        let own_instance = self.instance_name.clone();

        tokio::spawn(async move {
            info!("Started mDNS browsing for {}", MDNS_SERVICE_TYPE);

            loop {
                match receiver.recv_async().await {
                    Ok(event) => {
                        if let Err(e) =
                            handle_mdns_event(&event, &peers, &event_tx, &own_instance).await
                        {
                            warn!("Error handling mDNS event: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("mDNS browse channel closed: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Get a snapshot of currently known peers.
    pub fn peers(&self) -> Vec<PeerInfo> {
        self.peers
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }

    /// Unregister the service and stop the daemon.
    pub fn shutdown(self) -> Result<()> {
        if self.registered {
            let _ = self.daemon.unregister(&self.instance_name);
        }
        let _ = self.daemon.shutdown();
        info!("mDNS discovery service shut down");
        Ok(())
    }
}

/// Process an individual mDNS event.
async fn handle_mdns_event(
    event: &ServiceEvent,
    peers: &Arc<Mutex<HashMap<String, PeerInfo>>>,
    event_tx: &mpsc::Sender<DiscoveryEvent>,
    own_instance: &str,
) -> Result<()> {
    match event {
        ServiceEvent::ServiceResolved(info) => {
            let full_name = info.get_fullname().to_string();

            // Don't discover ourselves
            if full_name.contains(own_instance) {
                debug!("Ignoring own service: {}", full_name);
                return Ok(());
            }

            let port = info.get_port();
            let hostname = info
                .get_properties()
                .get("hostname")
                .map(|v| v.val_str().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let is_host = info
                .get_properties()
                .get("role")
                .map(|v| v.val_str() == "host")
                .unwrap_or(false);

            // Get the first available IP address
            let addresses = info.get_addresses();
            let ip = match addresses.iter().next() {
                Some(addr) => *addr,
                None => {
                    warn!("Service {} has no addresses", full_name);
                    return Ok(());
                }
            };

            let addr = SocketAddr::new(ip, port);
            let peer = PeerInfo {
                hostname: hostname.clone(),
                addr,
                is_host,
            };

            info!("Discovered peer: {} at {} (host={})", hostname, addr, is_host);

            {
                peers.lock().unwrap().insert(full_name.clone(), peer.clone());
            }
            let _ = event_tx.send(DiscoveryEvent::PeerDiscovered(peer)).await;
        }
        ServiceEvent::ServiceRemoved(_, full_name) => {
            info!("Peer lost: {}", full_name);

            let removed_peer = {
                peers.lock().unwrap().remove(full_name)
            };
            if let Some(peer) = removed_peer {
                let _ = event_tx
                    .send(DiscoveryEvent::PeerLost(peer.hostname))
                    .await;
            }
        }
        ServiceEvent::SearchStarted(service_type) => {
            debug!("mDNS search started for {}", service_type);
        }
        _ => {
            debug!("mDNS event: {:?}", event);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mdns_registration() {
        let (tx, _rx) = mpsc::channel(10);
        let mut ds = DiscoveryService::new("test-host", 9876, tx).expect("create DiscoveryService");
        let res = ds.register(9876, true);
        println!("Registration result: {:?}", res);
        assert!(res.is_ok(), "mDNS register failed: {:?}", res.err());
    }

    #[tokio::test]
    async fn test_mdns_browse_then_register() {
        let (tx1, _rx1) = mpsc::channel(10);
        let ds1 = DiscoveryService::new("browser", 9876, tx1).expect("create ds1");
        ds1.start_browsing().expect("start browsing");

        let (tx2, _rx2) = mpsc::channel(10);
        let config = screenextend_common::config::AppConfig::default();
        let mut ds2 = DiscoveryService::new(&config.hostname, 9876, tx2).expect("create ds2");
        let res = ds2.register(9876, true);
        println!("Browse then register result: {:?}", res);
        assert!(res.is_ok(), "browse then register failed: {:?}", res.err());
    }
}
