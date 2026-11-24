//! Network subsystem for `rust-micro-os`.
//!
//! Provides Ethernet framing, ARP, IPv4, UDP, ICMP Ping, DHCP, DNS, TCP, and HTTP client integration.

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
pub mod tcp;
pub mod http;
pub mod wifi;

pub use e1000::{E1000Device, E1000_DEVICE};
pub use ethernet::{EthernetHeader, MacAddress, BROADCAST_MAC, ETHERTYPE_ARP, ETHERTYPE_IPV4};
pub use arp::{ArpPacket, ArpEntry, ArpCache, ARP_CACHE};
pub use ipv4::{Ipv4Header, Ipv4Address, IP_PROTOCOL_ICMP, IP_PROTOCOL_TCP, IP_PROTOCOL_UDP, parse_ipv4_addr};
pub use udp::{UdpHeader, MAX_UDP_PAYLOAD_LEN};
pub use icmp::{IcmpEchoHeader, ICMP_TYPE_ECHO_REPLY, ICMP_TYPE_ECHO_REQUEST};
pub use config::{NetworkConfig, ConfigMode, get_config, set_mac_address, set_static_config, apply_dhcp_lease};
pub use dhcp::{DhcpLease, ParsedDhcpResponse, build_dhcp_discover, build_dhcp_request};
pub use dns::{DnsCache, DNS_CACHE, build_dns_query, parse_dns_response};
pub use tcp::{
    TcpHeader, TcpState, TcpStream, tcp_connect,
    TCP_FLAG_SYN, TCP_FLAG_ACK, TCP_FLAG_FIN, TCP_FLAG_RST, TCP_FLAG_PSH,
};
pub use http::{HttpResponse, build_http_get_request, parse_http_response, MAX_HTTP_BODY_LEN};
pub use stack::{
    process_incoming_frame, poll_network, resolve_arp, send_ipv4, send_udp,
    send_broadcast_udp, ping, acquire_dhcp_lease, resolve_dns,
    LAST_UDP, LAST_ICMP_REPLY, LAST_DHCP_RESPONSE, LAST_DNS_RESPONSE,
};

/// Initializes the network subsystem: initializes hardware driver, synchronizes MAC configuration,
/// and performs initial DHCP lease acquisition if link is active.
pub fn init() -> Result<(), &'static str> {
    e1000::init()?;
    if let Some(mac) = get_mac_address() {
        config::set_mac_address(mac);
    }

    if is_link_up() {
        crate::info!("[NET-INIT] E1000 hardware link active. Attempting startup DHCP exchange...");
        match renew_dhcp(2_500_000) {
            Ok(lease) => {
                crate::info!(
                    "[DHCP] Startup lease acquired: IP {}.{}.{}.{} | GW {}.{}.{}.{} | DNS {}.{}.{}.{}",
                    lease.assigned_ip[0], lease.assigned_ip[1], lease.assigned_ip[2], lease.assigned_ip[3],
                    lease.gateway[0], lease.gateway[1], lease.gateway[2], lease.gateway[3],
                    lease.dns_server[0], lease.dns_server[1], lease.dns_server[2], lease.dns_server[3],
                );
            }
            Err(e) => {
                crate::warn!("[DHCP] Startup lease timed out ({}); preserving static fallback.", e);
            }
        }
    } else {
        crate::warn!("[NET-INIT] E1000 link DOWN or device not present; network remaining offline.");
    }

    Ok(())
}

/// Returns true if the Intel E1000 hardware device is detected and has active physical link.
pub fn is_link_up() -> bool {
    let guard = E1000_DEVICE.lock();
    guard.as_ref().map(|d| d.is_link_up()).unwrap_or(false)
}

/// Returns true if an Intel E1000 network adapter is detected on the PCI bus.
pub fn is_device_present() -> bool {
    E1000_DEVICE.lock().is_some()
}

/// Sends an ICMP Echo Request to the default gateway with a bounded timeout.
pub fn ping_gateway(timeout_spins: usize) -> Result<usize, &'static str> {
    let gw = config::get_config().gateway;
    stack::ping(gw, timeout_spins)
}

/// Tests DNS name resolution against the configured DNS server using "google.com".
pub fn test_dns_resolution(timeout_spins: usize) -> Result<Ipv4Address, &'static str> {
    stack::resolve_dns("google.com", timeout_spins)
}

/// Dynamically renews or acquires an IPv4 DHCP lease.
pub fn renew_dhcp(timeout_spins: usize) -> Result<dhcp::DhcpLease, &'static str> {
    {
        let mut cfg = config::NETWORK_CONFIG.lock();
        cfg.mode = config::ConfigMode::DhcpPending;
    }
    match stack::acquire_dhcp_lease(timeout_spins) {
        Ok(lease) => Ok(lease),
        Err(e) => {
            config::set_dhcp_failed();
            Err(e)
        }
    }
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

fn parse_u16_str(s: &str) -> Option<u16> {
    let mut val: u32 = 0;
    if s.is_empty() {
        return None;
    }
    for b in s.bytes() {
        if !b.is_ascii_digit() {
            return None;
        }
        val = val * 10 + (b - b'0') as u32;
        if val > 65535 {
            return None;
        }
    }
    Some(val as u16)
}

/// Performs a complete HTTP/1.1 GET request to `target` (hostname or host:port) and `path`.
pub fn http_get(target: &str, path: &str) -> Result<HttpResponse, &'static str> {
    http_get_timeout(target, path, 15_000_000, 20_000_000)
}

/// Performs an HTTP/1.1 GET request with custom connect and read timeouts.
pub fn http_get_timeout(
    target: &str,
    path: &str,
    connect_timeout: usize,
    read_timeout: usize,
) -> Result<HttpResponse, &'static str> {
    // 1. Parse host and optional port (e.g. "example.com" or "10.0.2.2:8080")
    let (hostname, port) = if let Some(colon) = target.rfind(':') {
        let (h, p_str) = target.split_at(colon);
        let port_val = parse_u16_str(&p_str[1..]).unwrap_or(80);
        (h, port_val)
    } else {
        (target, 80)
    };

    // 2. Resolve host IP (check if already an IP address, or resolve via DNS)
    let remote_ip = match parse_ipv4_addr(hostname) {
        Some(ip) => ip,
        None => resolve_dns(hostname, connect_timeout)?,
    };

    // 3. Establish TCP connection
    let mut stream = tcp_connect(remote_ip, port, connect_timeout)?;

    // 4. Format and send HTTP GET request
    let mut req_buf = [0u8; 1024];
    let req_len = build_http_get_request(hostname, path, &mut req_buf)?;
    stream.write(&req_buf[..req_len])?;

    // 5. Read response stream
    let mut resp_raw = [0u8; 16384];
    let bytes_read = stream.read_to_end(&mut resp_raw, read_timeout)?;

    // 6. Parse response
    parse_http_response(&resp_raw[..bytes_read])
}
