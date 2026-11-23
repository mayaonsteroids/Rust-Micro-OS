//! Network Protocol Stack and Packet Dispatcher.
//!
//! Orchestrates Ethernet, ARP, IPv4, ICMP, and UDP frame processing.
//! Provides synchronous ARP resolution, UDP transmission, and ICMP Ping.

use spin::Mutex;
use super::ethernet::{EthernetHeader, MacAddress, BROADCAST_MAC, ETHERTYPE_ARP, ETHERTYPE_IPV4};
use super::arp::{ArpPacket, ARP_OPCODE_REQUEST, ARP_CACHE};
use super::ipv4::{Ipv4Header, Ipv4Address, IP_PROTOCOL_ICMP, IP_PROTOCOL_UDP, IPV4_BROADCAST};
use super::udp::UdpHeader;
use super::icmp::{IcmpEchoHeader, ICMP_TYPE_ECHO_REPLY, ICMP_TYPE_ECHO_REQUEST, ICMP_CODE_DEFAULT};
use super::config::get_config;

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

/// Global packet identification counter for IPv4 headers.
static IP_PACKET_ID: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(1);

/// Processes a single incoming raw Ethernet frame received from the hardware driver.
///
/// Dispatches to ARP, IPv4, ICMP, or UDP handlers with strict bounds checking.
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
                // Ignore packets not destined for us or broadcast
                if ip_header.dst_ip != cfg.ip && ip_header.dst_ip != IPV4_BROADCAST {
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
                            // Check checksum
                            if UdpHeader::verify_checksum(&ip_header.src_ip, &ip_header.dst_ip, ip_payload) {
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

/// Polls the network hardware for any incoming frames and processes them.
///
/// Returns the number of packets processed during this poll.
pub fn poll_network() -> usize {
    let mut count = 0;
    let mut rx_buf = [0u8; 1518];

    // Poll up to 16 frames per call to prevent starvation
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
///
/// First checks the local ARP cache. If not found, broadcasts an ARP request
/// and polls for the ARP reply within a bounded timeout.
pub fn resolve_arp(target_ip: Ipv4Address, timeout_spins: usize) -> Result<MacAddress, &'static str> {
    // 1. Check cache
    if let Some(mac) = ARP_CACHE.lock().lookup(&target_ip) {
        return Ok(mac);
    }

    let cfg = get_config();

    // 2. Build ARP request
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

    // 3. Transmit ARP request
    super::send_packet(&frame[..frame_len])?;

    // 4. Poll incoming frames until target IP is resolved or timeout expires
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

    // Final check
    if let Some(mac) = ARP_CACHE.lock().lookup(&target_ip) {
        Ok(mac)
    } else {
        Err("ARP resolution timed out: no reply received from target")
    }
}

/// Transmits an IPv4 packet, automatically resolving destination or gateway MAC address.
pub fn send_ipv4(
    dst_ip: Ipv4Address,
    protocol: u8,
    payload: &[u8],
) -> Result<(), &'static str> {
    let cfg = get_config();

    // Determine target IP for ARP resolution (local host or default gateway)
    let arp_target = if cfg.is_local_subnet(&dst_ip) || dst_ip == IPV4_BROADCAST {
        dst_ip
    } else {
        cfg.gateway
    };

    // If destination is broadcast, use broadcast MAC directly
    let dest_mac = if dst_ip == IPV4_BROADCAST {
        BROADCAST_MAC
    } else {
        resolve_arp(arp_target, 200_000)?
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

/// Sends an ICMP Echo Request and waits for the matching ICMP Echo Reply.
///
/// Returns the round-trip poll spin count on success.
pub fn ping(dst_ip: Ipv4Address, timeout_spins: usize) -> Result<usize, &'static str> {
    // Clear previous ICMP reply
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

    // Transmit ICMP Echo Request
    send_ipv4(dst_ip, IP_PROTOCOL_ICMP, &icmp_buf[..icmp_len])?;

    // Poll for Echo Reply
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
