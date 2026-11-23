//! Network subsystem for `rust-micro-os`.
//!
//! Provides Ethernet framing, ARP, IPv4, UDP, ICMP Ping, DHCP, DNS, and Intel E1000 driver integration.

pub mod e1000;
pub mod ethernet;
pub mod arp;
pub mod ipv4;
pub mod udp;
pub mod icmp;
pub mod config;
pub mod stack;
pub mod dhcp;
pub mod dns;

pub use e1000::{E1000Device, E1000_DEVICE};
pub use ethernet::{EthernetHeader, MacAddress, BROADCAST_MAC, ETHERTYPE_ARP, ETHERTYPE_IPV4};
pub use arp::{ArpPacket, ArpEntry, ArpCache, ARP_CACHE};
pub use ipv4::{Ipv4Header, Ipv4Address, IP_PROTOCOL_ICMP, IP_PROTOCOL_UDP};
pub use udp::{UdpHeader, MAX_UDP_PAYLOAD_LEN};
pub use icmp::{IcmpEchoHeader, ICMP_TYPE_ECHO_REPLY, ICMP_TYPE_ECHO_REQUEST};
pub use config::{NetworkConfig, ConfigMode, get_config, set_mac_address, set_static_config, apply_dhcp_lease};
pub use dhcp::{DhcpLease, ParsedDhcpResponse, build_dhcp_discover, build_dhcp_request};
pub use dns::{DnsCache, DNS_CACHE, build_dns_query, parse_dns_response};
pub use stack::{
    process_incoming_frame, poll_network, resolve_arp, send_ipv4, send_udp,
    send_broadcast_udp, ping, acquire_dhcp_lease, resolve_dns,
    LAST_UDP, LAST_ICMP_REPLY, LAST_DHCP_RESPONSE, LAST_DNS_RESPONSE,
};

/// Initializes the network subsystem: initializes hardware driver and synchronizes MAC configuration.
pub fn init() -> Result<(), &'static str> {
    e1000::init()?;
    if let Some(mac) = get_mac_address() {
        config::set_mac_address(mac);
    }
    Ok(())
}

/// Transmits a raw Ethernet frame via the active network controller.
pub fn send_packet(packet: &[u8]) -> Result<(), &'static str> {
    let mut guard = E1000_DEVICE.lock();
    let dev = guard.as_mut().ok_or("Network device not initialized")?;
    dev.transmit(packet)
}

/// Polls for an incoming Ethernet frame into `buf`.
pub fn receive_packet(buf: &mut [u8]) -> Option<usize> {
    let mut guard = E1000_DEVICE.lock();
    let dev = guard.as_mut()?;
    dev.receive(buf)
}

/// Retrieves the hardware MAC address of the active network controller.
pub fn get_mac_address() -> Option<[u8; 6]> {
    let guard = E1000_DEVICE.lock();
    let dev = guard.as_ref()?;
    Some(dev.mac_addr)
}

/// Retrieves network transmission and reception statistics:
/// (tx_packets, tx_bytes, rx_packets, rx_bytes)
pub fn get_statistics() -> Option<(u64, u64, u64, u64)> {
    let guard = E1000_DEVICE.lock();
    let dev = guard.as_ref()?;
    Some((
        dev.tx_packet_count,
        dev.tx_bytes_count,
        dev.rx_packet_count,
        dev.rx_bytes_count,
    ))
}
