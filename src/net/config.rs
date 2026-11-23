//! Network Configuration and Interface State.
//!
//! Stores the guest MAC address, IPv4 address, default gateway, subnet mask,
//! and DNS server. Distinguishes static configuration from future DHCP acquisition.

use spin::Mutex;
use super::ethernet::MacAddress;
use super::ipv4::Ipv4Address;

/// Active configuration mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigMode {
    Static,
    DhcpPending,
    DhcpConfigured,
}

/// Global Network Interface Configuration.
#[derive(Debug, Clone, Copy)]
pub struct NetworkConfig {
    pub mac: MacAddress,
    pub ip: Ipv4Address,
    pub subnet_mask: Ipv4Address,
    pub gateway: Ipv4Address,
    pub dns: Ipv4Address,
    pub mode: ConfigMode,
}

impl NetworkConfig {
    /// Constructs default QEMU user-mode networking static configuration:
    /// - Guest IP: 10.0.2.15
    /// - Subnet Mask: 255.255.255.0
    /// - Gateway: 10.0.2.2
    /// - DNS Server: 10.0.2.3
    pub const fn default_qemu(mac: MacAddress) -> Self {
        Self {
            mac,
            ip: [10, 0, 2, 15],
            subnet_mask: [255, 255, 255, 0],
            gateway: [10, 0, 2, 2],
            dns: [10, 0, 2, 3],
            mode: ConfigMode::Static,
        }
    }

    /// Checks if a destination IP address resides on the local subnet.
    pub fn is_local_subnet(&self, dest_ip: &Ipv4Address) -> bool {
        for i in 0..4 {
            if (self.ip[i] & self.subnet_mask[i]) != (dest_ip[i] & self.subnet_mask[i]) {
                return false;
            }
        }
        true
    }
}

pub static NETWORK_CONFIG: Mutex<NetworkConfig> = Mutex::new(NetworkConfig {
    mac: [0; 6],
    ip: [10, 0, 2, 15],
    subnet_mask: [255, 255, 255, 0],
    gateway: [10, 0, 2, 2],
    dns: [10, 0, 2, 3],
    mode: ConfigMode::Static,
});

/// Updates the MAC address in the global network configuration.
pub fn set_mac_address(mac: MacAddress) {
    let mut cfg = NETWORK_CONFIG.lock();
    cfg.mac = mac;
}

/// Returns a copy of the active network configuration.
pub fn get_config() -> NetworkConfig {
    *NETWORK_CONFIG.lock()
}

/// Sets static IPv4 parameters.
pub fn set_static_config(ip: Ipv4Address, subnet_mask: Ipv4Address, gateway: Ipv4Address, dns: Ipv4Address) {
    let mut cfg = NETWORK_CONFIG.lock();
    cfg.ip = ip;
    cfg.subnet_mask = subnet_mask;
    cfg.gateway = gateway;
    cfg.dns = dns;
    cfg.mode = ConfigMode::Static;
}
