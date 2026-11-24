//! Network Protocol Stack and Packet Dispatcher.
//!
//! Orchestrates Ethernet, ARP, IPv4, ICMP, UDP, DHCP, and DNS packet processing.
//! Provides synchronous ARP resolution, UDP transmission, ICMP Ping,
//! DHCP lease acquisition, and DNS name resolution.

use spin::Mutex;
use super::ethernet::{EthernetHeader, MacAddress, BROADCAST_MAC, ETHERTYPE_ARP, ETHERTYPE_IPV4};
use super::arp::{ArpPacket, ARP_OPCODE_REQUEST, ARP_CACHE};
use super::ipv4::{Ipv4Header, Ipv4Address, IP_PROTOCOL_ICMP, IP_PROTOCOL_UDP, IPV4_BROADCAST};
use super::udp::UdpHeader;
use super::icmp::{IcmpEchoHeader, ICMP_TYPE_ECHO_REPLY, ICMP_TYPE_ECHO_REQUEST, ICMP_CODE_DEFAULT};
use super::dhcp::{
    DhcpLease, ParsedDhcpResponse, DHCP_CLIENT_PORT, DHCP_SERVER_PORT,
    DHCP_MSG_OFFER, DHCP_MSG_ACK, build_dhcp_discover, build_dhcp_request,
};
use super::dns::{DNS_PORT, build_dns_query, parse_dns_response, DNS_CACHE};
use super::config::{get_config, apply_dhcp_lease};

pub const DNS_CLIENT_PORT: u16 = 53535;

/// Bounded storage for the most recently received UDP datagram.
#[derive(Debug, Clone, Copy)]
pub struct ReceivedUdp {
    pub src_ip: Ipv4Address,
    pub dst_ip: Ipv4Address,
    pub src_port: u16,
    pub dst_port: u16,
    pub length: usize,
    pub payload: [u8; 256],
}

pub static LAST_UDP: Mutex<Option<ReceivedUdp>> = Mutex::new(None);

/// Bounded storage for the most recently received ICMP Echo Reply.
#[derive(Debug, Clone, Copy)]
pub struct ReceivedIcmpEcho {
    pub src_ip: Ipv4Address,
    pub identifier: u16,
    pub sequence_number: u16,
    pub payload_len: usize,
}

pub static LAST_ICMP_REPLY: Mutex<Option<ReceivedIcmpEcho>> = Mutex::new(None);

/// Bounded storage for incoming DHCP server responses.
pub static LAST_DHCP_RESPONSE: Mutex<Option<ParsedDhcpResponse>> = Mutex::new(None);

/// Bounded storage for incoming DNS server responses (xid, buffer, length).
pub static LAST_DNS_RESPONSE: Mutex<Option<(u16, [u8; 512], usize)>> = Mutex::new(None);

/// Global packet identification counter for IPv4 headers.
static IP_PACKET_ID: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(1);
static DHCP_XID_COUNTER: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0x3903F320);
static DNS_XID_COUNTER: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(0x5430);

/// Processes a single incoming raw Ethernet frame received from the hardware driver.
///
/// Dispatches to ARP, IPv4, ICMP, UDP, DHCP, or DNS handlers with strict bounds checking.
pub fn process_incoming_frame(frame: &[u8]) {
    let (eth_header, eth_payload) = match EthernetHeader::parse(frame) {
        Ok(res) => res,
        Err(_) => return, // Drop malformed frame
    };

    let cfg = get_config();

    match eth_header.ethertype {
        ETHERTYPE_ARP => {
            if let Ok(arp) = ArpPacket::parse(eth_payload) {
                // Record sender in ARP cache
                ARP_CACHE.lock().insert(arp.sender_ip, arp.sender_mac);

                // If this is an ARP Request for our IP address, generate and send an ARP Reply
                if arp.opcode == ARP_OPCODE_REQUEST && arp.target_ip == cfg.ip {
                    let mut reply_payload = [0u8; 28];
                    if ArpPacket::build_reply(
                        cfg.mac,
                        cfg.ip,
                        arp.sender_mac,
                        arp.sender_ip,
                        &mut reply_payload,
                    ).is_ok() {
                        let mut out_frame = [0u8; 60];
                        if let Ok(frame_len) = EthernetHeader::build_frame(
                            arp.sender_mac,
                            cfg.mac,
                            ETHERTYPE_ARP,
                            &reply_payload,
                            &mut out_frame,
                        ) {
                            let _ = super::send_packet(&out_frame[..frame_len]);
                        }
                    }
                }
            }
        }
        ETHERTYPE_IPV4 => {
            if let Ok((ip_header, ip_payload)) = Ipv4Header::parse(eth_payload) {
                // Filter packets: accept if destined for our IP, broadcast, or directed to our MAC
                let is_for_us = ip_header.dst_ip == cfg.ip
                    || ip_header.dst_ip == IPV4_BROADCAST
                    || eth_header.dest_mac == cfg.mac
                    || eth_header.dest_mac == BROADCAST_MAC;

                if !is_for_us {
                    return;
                }

                match ip_header.protocol {
                    IP_PROTOCOL_ICMP => {
                        if let Ok((icmp_header, icmp_payload)) = IcmpEchoHeader::parse(ip_payload) {
                            if icmp_header.icmp_type == ICMP_TYPE_ECHO_REQUEST {
                                // Echo Request: Reply back
                                let mut reply_icmp = [0u8; 128];
                                if let Ok(icmp_len) = IcmpEchoHeader::build_message(
                                    ICMP_TYPE_ECHO_REPLY,
                                    ICMP_CODE_DEFAULT,
                                    icmp_header.identifier,
                                    icmp_header.sequence_number,
                                    icmp_payload,
                                    &mut reply_icmp,
                                ) {
                                    let mut ip_buf = [0u8; 160];
                                    let id = IP_PACKET_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                                    if let Ok(ip_len) = Ipv4Header::build_packet(
                                        cfg.ip,
                                        ip_header.src_ip,
                                        IP_PROTOCOL_ICMP,
                                        id,
                                        &reply_icmp[..icmp_len],
                                        &mut ip_buf,
                                    ) {
                                        let mut eth_buf = [0u8; 200];
                                        if let Ok(eth_len) = EthernetHeader::build_frame(
                                            eth_header.src_mac,
                                            cfg.mac,
                                            ETHERTYPE_IPV4,
                                            &ip_buf[..ip_len],
                                            &mut eth_buf,
                                        ) {
                                            let _ = super::send_packet(&eth_buf[..eth_len]);
                                        }
                                    }
                                }
                            } else if icmp_header.icmp_type == ICMP_TYPE_ECHO_REPLY {
                                *LAST_ICMP_REPLY.lock() = Some(ReceivedIcmpEcho {
                                    src_ip: ip_header.src_ip,
                                    identifier: icmp_header.identifier,
                                    sequence_number: icmp_header.sequence_number,
                                    payload_len: icmp_payload.len(),
                                });
                            }
                        }
                    }
                    IP_PROTOCOL_UDP => {
                        if let Ok((udp_header, udp_payload)) = UdpHeader::parse(ip_payload) {
                            // Check for DHCP client port (68)
                            if udp_header.dst_port == DHCP_CLIENT_PORT {
                                if let Ok(dhcp_resp) = ParsedDhcpResponse::parse(udp_payload) {
                                    *LAST_DHCP_RESPONSE.lock() = Some(dhcp_resp);
                                }
                            }
                            // Check for DNS response (from DNS port 53 or directed to client port 53535)
                            else if udp_header.dst_port == DNS_CLIENT_PORT || udp_header.src_port == DNS_PORT {
                                if udp_payload.len() >= 12 {
                                    let xid = ((udp_payload[0] as u16) << 8) | (udp_payload[1] as u16);
                                    let mut buf = [0u8; 512];
                                    let copy_len = core::cmp::min(udp_payload.len(), 512);
                                    buf[..copy_len].copy_from_slice(&udp_payload[..copy_len]);
                                    *LAST_DNS_RESPONSE.lock() = Some((xid, buf, copy_len));
                                }
                            }
                            // General UDP datagram
                            else if UdpHeader::verify_checksum(&ip_header.src_ip, &ip_header.dst_ip, ip_payload) {
                                let mut p_buf = [0u8; 256];
                                let copy_len = core::cmp::min(udp_payload.len(), 256);
                                p_buf[..copy_len].copy_from_slice(&udp_payload[..copy_len]);

                                *LAST_UDP.lock() = Some(ReceivedUdp {
                                    src_ip: ip_header.src_ip,
                                    dst_ip: ip_header.dst_ip,
                                    src_port: udp_header.src_port,
                                    dst_port: udp_header.dst_port,
                                    length: udp_payload.len(),
                                    payload: p_buf,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

/// Polls the network hardware for incoming frames and processes them.
pub fn poll_network() -> usize {
    let mut count = 0;
    let mut rx_buf = [0u8; 1518];

    for _ in 0..16 {
        if let Some(len) = super::receive_packet(&mut rx_buf) {
            process_incoming_frame(&rx_buf[..len]);
            count += 1;
        } else {
            break;
        }
    }

    count
}

/// Resolves an IPv4 address to an Ethernet MAC address via ARP.
pub fn resolve_arp(target_ip: Ipv4Address, timeout_spins: usize) -> Result<MacAddress, &'static str> {
    if let Some(mac) = ARP_CACHE.lock().lookup(&target_ip) {
        return Ok(mac);
    }

    let cfg = get_config();

    let mut arp_payload = [0u8; 28];
    ArpPacket::build_request(cfg.mac, cfg.ip, target_ip, &mut arp_payload)?;

    let mut frame = [0u8; 60];
    let frame_len = EthernetHeader::build_frame(
        BROADCAST_MAC,
        cfg.mac,
        ETHERTYPE_ARP,
        &arp_payload,
        &mut frame,
    )?;

    super::send_packet(&frame[..frame_len])?;

    let mut rx_buf = [0u8; 1518];
    for _ in 0..timeout_spins {
        if let Some(len) = super::receive_packet(&mut rx_buf) {
            process_incoming_frame(&rx_buf[..len]);
            if let Some(mac) = ARP_CACHE.lock().lookup(&target_ip) {
                return Ok(mac);
            }
        }
        core::hint::spin_loop();
    }

    if let Some(mac) = ARP_CACHE.lock().lookup(&target_ip) {
        Ok(mac)
    } else {
        Err("ARP resolution timed out: no reply received from target")
    }
}

/// Transmits an IPv4 packet, resolving destination or gateway MAC address.
pub fn send_ipv4(
    dst_ip: Ipv4Address,
    protocol: u8,
    payload: &[u8],
) -> Result<(), &'static str> {
    let cfg = get_config();

    let arp_target = if cfg.is_local_subnet(&dst_ip) || dst_ip == IPV4_BROADCAST {
        dst_ip
    } else {
        cfg.gateway
    };

    let dest_mac = if dst_ip == IPV4_BROADCAST {
        BROADCAST_MAC
    } else {
        resolve_arp(arp_target, 2_000_000)?
    };

    let mut ip_buf = [0u8; 1500];
    let id = IP_PACKET_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    let ip_len = Ipv4Header::build_packet(
        cfg.ip,
        dst_ip,
        protocol,
        id,
        payload,
        &mut ip_buf,
    )?;

    let mut eth_buf = [0u8; 1518];
    let eth_len = EthernetHeader::build_frame(
        dest_mac,
        cfg.mac,
        ETHERTYPE_IPV4,
        &ip_buf[..ip_len],
        &mut eth_buf,
    )?;

    super::send_packet(&eth_buf[..eth_len])
}

/// Transmits a UDP datagram to the specified destination IPv4 and port.
pub fn send_udp(
    dst_ip: Ipv4Address,
    dst_port: u16,
    src_port: u16,
    payload: &[u8],
) -> Result<(), &'static str> {
    let cfg = get_config();

    let mut udp_buf = [0u8; 1472];
    let udp_len = UdpHeader::build_packet(
        &cfg.ip,
        &dst_ip,
        src_port,
        dst_port,
        payload,
        &mut udp_buf,
    )?;

    send_ipv4(dst_ip, IP_PROTOCOL_UDP, &udp_buf[..udp_len])
}

/// Transmits a broadcast UDP datagram with source IP 0.0.0.0 and destination 255.255.255.255.
///
/// Specifically utilized for standard BOOTP/DHCP client discovery and requests.
pub fn send_broadcast_udp(
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Result<(), &'static str> {
    let cfg = get_config();
    let src_ip = [0, 0, 0, 0];
    let dst_ip = IPV4_BROADCAST;

    let mut udp_buf = [0u8; 1472];
    let udp_len = UdpHeader::build_packet(
        &src_ip,
        &dst_ip,
        src_port,
        dst_port,
        payload,
        &mut udp_buf,
    )?;

    let mut ip_buf = [0u8; 1500];
    let id = IP_PACKET_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    let ip_len = Ipv4Header::build_packet(
        src_ip,
        dst_ip,
        IP_PROTOCOL_UDP,
        id,
        &udp_buf[..udp_len],
        &mut ip_buf,
    )?;

    let mut eth_buf = [0u8; 1518];
    let eth_len = EthernetHeader::build_frame(
        BROADCAST_MAC,
        cfg.mac,
        ETHERTYPE_IPV4,
        &ip_buf[..ip_len],
        &mut eth_buf,
    )?;

    super::send_packet(&eth_buf[..eth_len])
}

/// Sends an ICMP Echo Request and waits for the matching ICMP Echo Reply.
pub fn ping(dst_ip: Ipv4Address, timeout_spins: usize) -> Result<usize, &'static str> {
    *LAST_ICMP_REPLY.lock() = None;

    let ident = 0x1234;
    let seq = 1;
    let ping_data = b"MicroOS Ping Payload";

    let mut icmp_buf = [0u8; 64];
    let icmp_len = IcmpEchoHeader::build_message(
        ICMP_TYPE_ECHO_REQUEST,
        ICMP_CODE_DEFAULT,
        ident,
        seq,
        ping_data,
        &mut icmp_buf,
    )?;

    send_ipv4(dst_ip, IP_PROTOCOL_ICMP, &icmp_buf[..icmp_len])?;

    let mut rx_buf = [0u8; 1518];
    for spin in 0..timeout_spins {
        if let Some(len) = super::receive_packet(&mut rx_buf) {
            process_incoming_frame(&rx_buf[..len]);

            if let Some(reply) = *LAST_ICMP_REPLY.lock() {
                if reply.src_ip == dst_ip && reply.identifier == ident && reply.sequence_number == seq {
                    return Ok(spin);
                }
            }
        }
        core::hint::spin_loop();
    }

    Err("Ping request timed out: no ICMP Echo Reply received")
}

/// Executes a full DHCP 4-way handshake (DISCOVER -> OFFER -> REQUEST -> ACK)
/// to dynamically acquire an IPv4 lease from the local network.
pub fn acquire_dhcp_lease(timeout_spins: usize) -> Result<DhcpLease, &'static str> {
    let cfg = get_config();
    let xid = DHCP_XID_COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed);

    // 1. Send DHCPDISCOVER
    *LAST_DHCP_RESPONSE.lock() = None;
    let mut discover_buf = [0u8; 350];
    let disc_len = build_dhcp_discover(cfg.mac, xid, &mut discover_buf)?;
    send_broadcast_udp(DHCP_CLIENT_PORT, DHCP_SERVER_PORT, &discover_buf[..disc_len])?;

    // 2. Poll for DHCPOFFER
    let mut rx_buf = [0u8; 1518];
    let mut offer_found: Option<ParsedDhcpResponse> = None;
    for _ in 0..timeout_spins {
        if let Some(len) = super::receive_packet(&mut rx_buf) {
            process_incoming_frame(&rx_buf[..len]);
            if let Some(resp) = *LAST_DHCP_RESPONSE.lock() {
                // Validate XID, Client Hardware MAC address, and DHCPOFFER message type
                if resp.xid == xid && resp.chaddr == cfg.mac && resp.msg_type == Some(DHCP_MSG_OFFER) {
                    offer_found = Some(resp);
                    break;
                }
            }
        }
        core::hint::spin_loop();
    }

    let offer = offer_found.ok_or("DHCP timeout waiting for DHCPOFFER")?;
    let offered_ip = offer.yiaddr;
    let server_id = offer.server_id.unwrap_or([10, 0, 2, 2]);

    // 3. Send DHCPREQUEST
    *LAST_DHCP_RESPONSE.lock() = None;
    let mut request_buf = [0u8; 350];
    let req_len = build_dhcp_request(cfg.mac, xid, offered_ip, server_id, &mut request_buf)?;
    send_broadcast_udp(DHCP_CLIENT_PORT, DHCP_SERVER_PORT, &request_buf[..req_len])?;

    // 4. Poll for DHCPACK
    let mut ack_found: Option<ParsedDhcpResponse> = None;
    for _ in 0..timeout_spins {
        if let Some(len) = super::receive_packet(&mut rx_buf) {
            process_incoming_frame(&rx_buf[..len]);
            if let Some(resp) = *LAST_DHCP_RESPONSE.lock() {
                // Validate XID, Client Hardware MAC address, and DHCPACK message type
                if resp.xid == xid && resp.chaddr == cfg.mac && resp.msg_type == Some(DHCP_MSG_ACK) {
                    ack_found = Some(resp);
                    break;
                }
            }
        }
        core::hint::spin_loop();
    }

    let ack = ack_found.ok_or("DHCP timeout waiting for DHCPACK")?;

    let lease = DhcpLease {
        assigned_ip: ack.yiaddr,
        subnet_mask: ack.subnet_mask.unwrap_or([255, 255, 255, 0]),
        gateway: ack.router.unwrap_or([10, 0, 2, 2]),
        dns_server: ack.dns_server.unwrap_or([10, 0, 2, 3]),
        lease_time_secs: ack.lease_time.unwrap_or(86400),
        server_ip: server_id,
    };

    // Apply the validated lease to global configuration
    apply_dhcp_lease(&lease);

    Ok(lease)
}

/// Resolves a domain name into an IPv4 address using DNS over UDP port 53.
pub fn resolve_dns(domain: &str, timeout_spins: usize) -> Result<Ipv4Address, &'static str> {
    // 1. Check in-memory DNS cache
    if let Some(cached_ip) = DNS_CACHE.lock().lookup(domain) {
        return Ok(cached_ip);
    }

    let cfg = get_config();
    let dns_server = cfg.dns;

    let xid = DNS_XID_COUNTER.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    *LAST_DNS_RESPONSE.lock() = None;

    let mut query_buf = [0u8; 256];
    let query_len = build_dns_query(domain, xid, &mut query_buf)?;

    // Send DNS query UDP packet to DNS server:53 from ephemeral port 53535
    send_udp(dns_server, DNS_PORT, DNS_CLIENT_PORT, &query_buf[..query_len])?;

    // Poll for DNS response
    let mut rx_buf = [0u8; 1518];
    for _ in 0..timeout_spins {
        if let Some(len) = super::receive_packet(&mut rx_buf) {
            process_incoming_frame(&rx_buf[..len]);

            if let Some((resp_xid, ref resp_buf, resp_len)) = *LAST_DNS_RESPONSE.lock() {
                if resp_xid == xid {
                    let resolved_ip = parse_dns_response(&resp_buf[..resp_len], xid)?;
                    DNS_CACHE.lock().insert(domain, resolved_ip);
                    return Ok(resolved_ip);
                }
            }
        }
        core::hint::spin_loop();
    }

    Err("DNS resolution timed out: no response from DNS server")
}
