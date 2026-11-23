//! Transmission Control Protocol (TCP) Client Layer (RFC 793).
//!
//! Provides TCP header construction, bounds-checked parsing, RFC 793 checksum
//! calculation using IPv4 pseudo-headers, 3-way handshake, sequence/acknowledgement
//! tracking, connection state management, and synchronous stream I/O.

use core::sync::atomic::{AtomicU16, AtomicU32, Ordering};
use super::ethernet::{EthernetHeader, ETHERTYPE_IPV4};
use super::ipv4::{Ipv4Header, Ipv4Address, IP_PROTOCOL_TCP};
use super::config::get_config;

pub const TCP_HEADER_MIN_LEN: usize = 20;
pub const TCP_DEFAULT_WINDOW: u16 = 8192;
pub const TCP_MSS: u16 = 1460;

// TCP Control Flags
pub const TCP_FLAG_FIN: u8 = 1 << 0; // 0x01
pub const TCP_FLAG_SYN: u8 = 1 << 1; // 0x02
pub const TCP_FLAG_RST: u8 = 1 << 2; // 0x04
pub const TCP_FLAG_PSH: u8 = 1 << 3; // 0x08
pub const TCP_FLAG_ACK: u8 = 1 << 4; // 0x10
pub const TCP_FLAG_URG: u8 = 1 << 5; // 0x20

static TCP_LOCAL_PORT_COUNTER: AtomicU16 = AtomicU16::new(49200);
static TCP_SEQ_COUNTER: AtomicU32 = AtomicU32::new(0x2000_0000);

/// TCP Connection State Machine (Client perspective).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    SynSent,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
    Reset,
}

impl TcpState {
    pub fn as_str(&self) -> &'static str {
        match self {
            TcpState::Closed => "CLOSED",
            TcpState::SynSent => "SYN-SENT",
            TcpState::Established => "ESTABLISHED",
            TcpState::FinWait1 => "FIN-WAIT-1",
            TcpState::FinWait2 => "FIN-WAIT-2",
            TcpState::CloseWait => "CLOSE-WAIT",
            TcpState::Closing => "CLOSING",
            TcpState::LastAck => "LAST-ACK",
            TcpState::TimeWait => "TIME-WAIT",
            TcpState::Reset => "RESET",
        }
    }
}

/// Parsed TCP Segment Header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub data_offset: u8, // in bytes (IHL * 4)
    pub flags: u8,
    pub window_size: u16,
    pub checksum: u16,
    pub urgent_ptr: u16,
}

impl TcpHeader {
    /// Calculates the RFC 793 16-bit one's complement checksum using the IPv4 pseudo-header.
    pub fn calculate_checksum(
        src_ip: &Ipv4Address,
        dst_ip: &Ipv4Address,
        tcp_segment: &[u8],
    ) -> u16 {
        let mut sum: u32 = 0;

        // 1. IPv4 Pseudo-Header
        sum = sum.wrapping_add(((src_ip[0] as u32) << 8) | (src_ip[1] as u32));
        sum = sum.wrapping_add(((src_ip[2] as u32) << 8) | (src_ip[3] as u32));
        sum = sum.wrapping_add(((dst_ip[0] as u32) << 8) | (dst_ip[1] as u32));
        sum = sum.wrapping_add(((dst_ip[2] as u32) << 8) | (dst_ip[3] as u32));
        sum = sum.wrapping_add(6u32); // Protocol 6 (TCP)
        sum = sum.wrapping_add(tcp_segment.len() as u32);

        // 2. TCP Segment (checksum field at bytes 16..18 skipped/treated as zero)
        let words = tcp_segment.len() / 2;
        for i in 0..words {
            if i == 8 {
                continue;
            }
            let word = ((tcp_segment[i * 2] as u32) << 8) | (tcp_segment[i * 2 + 1] as u32);
            sum = sum.wrapping_add(word);
        }

        // Odd trailing byte if any
        if (tcp_segment.len() % 2) != 0 {
            let last_byte = tcp_segment[tcp_segment.len() - 1];
            sum = sum.wrapping_add((last_byte as u32) << 8);
        }

        // 3. Fold 32-bit carry bits into 16 bits
        while (sum >> 16) != 0 {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }

        !sum as u16
    }

    /// Verifies the checksum of an incoming TCP segment against its IPv4 pseudo-header.
    pub fn verify_checksum(
        src_ip: &Ipv4Address,
        dst_ip: &Ipv4Address,
        tcp_segment: &[u8],
    ) -> bool {
        if tcp_segment.len() < TCP_HEADER_MIN_LEN {
            return false;
        }
        let stored_checksum = ((tcp_segment[16] as u16) << 8) | (tcp_segment[17] as u16);
        let computed = Self::calculate_checksum(src_ip, dst_ip, tcp_segment);
        stored_checksum == computed
    }

    /// Parses a TCP segment from raw bytes with strict bounds checking.
    pub fn parse<'a>(raw: &'a [u8]) -> Result<(Self, &'a [u8]), &'static str> {
        if raw.len() < TCP_HEADER_MIN_LEN {
            return Err("Packet shorter than 20-byte TCP header");
        }

        let src_port = ((raw[0] as u16) << 8) | (raw[1] as u16);
        let dst_port = ((raw[2] as u16) << 8) | (raw[3] as u16);
        let seq_num = ((raw[4] as u32) << 24)
            | ((raw[5] as u32) << 16)
            | ((raw[6] as u32) << 8)
            | (raw[7] as u32);
        let ack_num = ((raw[8] as u32) << 24)
            | ((raw[9] as u32) << 16)
            | ((raw[10] as u32) << 8)
            | (raw[11] as u32);

        let data_offset_words = raw[12] >> 4;
        let data_offset = data_offset_words * 4;

        if (data_offset as usize) < TCP_HEADER_MIN_LEN {
            return Err("Invalid TCP data offset (must be at least 20 bytes)");
        }
        if raw.len() < (data_offset as usize) {
            return Err("TCP packet shorter than header length indicated by data offset");
        }

        let flags = raw[13];
        let window_size = ((raw[14] as u16) << 8) | (raw[15] as u16);
        let checksum = ((raw[16] as u16) << 8) | (raw[17] as u16);
        let urgent_ptr = ((raw[18] as u16) << 8) | (raw[19] as u16);

        let header = Self {
            src_port,
            dst_port,
            seq_num,
            ack_num,
            data_offset,
            flags,
            window_size,
            checksum,
            urgent_ptr,
        };

        let payload = &raw[data_offset as usize..];
        Ok((header, payload))
    }

    /// Serializes a TCP segment into `out_buf` with valid IPv4 pseudo-header checksum.
    pub fn build_segment(
        src_ip: &Ipv4Address,
        dst_ip: &Ipv4Address,
        src_port: u16,
        dst_port: u16,
        seq_num: u32,
        ack_num: u32,
        flags: u8,
        window_size: u16,
        with_mss: bool,
        payload: &[u8],
        out_buf: &mut [u8],
    ) -> Result<usize, &'static str> {
        let header_len = if with_mss { 24 } else { 20 };
        let total_len = header_len + payload.len();

        if out_buf.len() < total_len {
            return Err("Output buffer too small for TCP segment");
        }

        for b in &mut out_buf[..header_len] {
            *b = 0;
        }

        out_buf[0] = (src_port >> 8) as u8;
        out_buf[1] = (src_port & 0xFF) as u8;
        out_buf[2] = (dst_port >> 8) as u8;
        out_buf[3] = (dst_port & 0xFF) as u8;

        out_buf[4] = (seq_num >> 24) as u8;
        out_buf[5] = (seq_num >> 16) as u8;
        out_buf[6] = (seq_num >> 8) as u8;
        out_buf[7] = (seq_num & 0xFF) as u8;

        out_buf[8] = (ack_num >> 24) as u8;
        out_buf[9] = (ack_num >> 16) as u8;
        out_buf[10] = (ack_num >> 8) as u8;
        out_buf[11] = (ack_num & 0xFF) as u8;

        let offset_words = (header_len / 4) as u8;
        out_buf[12] = offset_words << 4;
        out_buf[13] = flags;

        out_buf[14] = (window_size >> 8) as u8;
        out_buf[15] = (window_size & 0xFF) as u8;

        if with_mss {
            out_buf[20] = 0x02; // Option Kind: MSS
            out_buf[21] = 0x04; // Option Length: 4
            out_buf[22] = (TCP_MSS >> 8) as u8;
            out_buf[23] = (TCP_MSS & 0xFF) as u8;
        }

        out_buf[header_len..total_len].copy_from_slice(payload);

        let checksum = Self::calculate_checksum(src_ip, dst_ip, &out_buf[..total_len]);
        out_buf[16] = (checksum >> 8) as u8;
        out_buf[17] = (checksum & 0xFF) as u8;

        Ok(total_len)
    }
}

/// Active connected TCP Client Stream.
pub struct TcpStream {
    pub remote_ip: Ipv4Address,
    pub remote_port: u16,
    pub local_port: u16,
    pub local_seq: u32,
    pub remote_seq: u32,
    pub state: TcpState,
}

impl TcpStream {
    /// Transmits a TCP segment for this connection over IPv4/Ethernet.
    fn send_segment(&self, flags: u8, with_mss: bool, payload: &[u8]) -> Result<(), &'static str> {
        let cfg = get_config();
        let mut tcp_buf = [0u8; 1500];
        let tcp_len = TcpHeader::build_segment(
            &cfg.ip,
            &self.remote_ip,
            self.local_port,
            self.remote_port,
            self.local_seq,
            self.remote_seq,
            flags,
            TCP_DEFAULT_WINDOW,
            with_mss,
            payload,
            &mut tcp_buf,
        )?;

        super::stack::send_ipv4(self.remote_ip, IP_PROTOCOL_TCP, &tcp_buf[..tcp_len])
    }

    /// Sends an ACK segment acknowledging current `remote_seq`.
    fn send_ack(&self) -> Result<(), &'static str> {
        self.send_segment(TCP_FLAG_ACK, false, &[])
    }

    /// Writes data over the established TCP connection (sets PSH | ACK).
    pub fn write(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if self.state != TcpState::Established {
            return Err("Cannot write to non-established TCP connection");
        }

        // Send data in chunks up to MSS (1460 bytes)
        let mut offset = 0;
        while offset < data.len() {
            let chunk_len = core::cmp::min(data.len() - offset, TCP_MSS as usize);
            let chunk = &data[offset..offset + chunk_len];

            self.send_segment(TCP_FLAG_PSH | TCP_FLAG_ACK, false, chunk)?;
            self.local_seq = self.local_seq.wrapping_add(chunk_len as u32);
            offset += chunk_len;
        }

        Ok(())
    }

    /// Reads incoming stream data into `out_buf` until the peer closes (FIN) or timeout occurs.
    pub fn read_to_end(&mut self, out_buf: &mut [u8], timeout_spins: usize) -> Result<usize, &'static str> {
        let mut total_read = 0;
        let mut rx_buf = [0u8; 1518];
        let mut idle_spins = 0;

        while idle_spins < timeout_spins {
            if let Some(frame_len) = super::receive_packet(&mut rx_buf) {
                idle_spins = 0;

                // Parse Ethernet header
                if let Ok((eth_header, eth_payload)) = EthernetHeader::parse(&rx_buf[..frame_len]) {
                    if eth_header.ethertype == ETHERTYPE_IPV4 {
                        // Parse IPv4 header
                        if let Ok((ip_header, ip_payload)) = Ipv4Header::parse(eth_payload) {
                            if ip_header.src_ip == self.remote_ip && ip_header.protocol == IP_PROTOCOL_TCP {
                                // Parse TCP header
                                if let Ok((tcp_header, tcp_payload)) = TcpHeader::parse(ip_payload) {
                                    if tcp_header.src_port == self.remote_port && tcp_header.dst_port == self.local_port {
                                        // Check for RST
                                        if (tcp_header.flags & TCP_FLAG_RST) != 0 {
                                            self.state = TcpState::Reset;
                                            return Err("TCP connection reset by peer (RST)");
                                        }

                                        // Handle incoming payload data
                                        if !tcp_payload.is_empty() {
                                            let copy_len = core::cmp::min(
                                                tcp_payload.len(),
                                                out_buf.len().saturating_sub(total_read),
                                            );
                                            out_buf[total_read..total_read + copy_len]
                                                .copy_from_slice(&tcp_payload[..copy_len]);
                                            total_read += copy_len;

                                            // Advance sequence number and ACK
                                            self.remote_seq = self.remote_seq.wrapping_add(tcp_payload.len() as u32);
                                            let _ = self.send_ack();
                                        }

                                        // Handle FIN
                                        if (tcp_header.flags & TCP_FLAG_FIN) != 0 {
                                            self.remote_seq = self.remote_seq.wrapping_add(1);
                                            let _ = self.send_ack(); // ACK the FIN
                                            let _ = self.send_segment(TCP_FLAG_FIN | TCP_FLAG_ACK, false, &[]);
                                            self.local_seq = self.local_seq.wrapping_add(1);
                                            self.state = TcpState::Closed;
                                            return Ok(total_read);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                idle_spins += 1;
                core::hint::spin_loop();
            }
        }

        if total_read > 0 {
            Ok(total_read)
        } else {
            Err("TCP read timed out: no data received from server")
        }
    }

    /// Closes the connection gracefully by sending FIN | ACK.
    pub fn close(&mut self) {
        if self.state == TcpState::Established {
            let _ = self.send_segment(TCP_FLAG_FIN | TCP_FLAG_ACK, false, &[]);
            self.local_seq = self.local_seq.wrapping_add(1);
            self.state = TcpState::FinWait1;
        }
    }
}

/// Connects to a remote TCP endpoint via standard 3-way handshake (SYN -> SYN-ACK -> ACK).
pub fn tcp_connect(
    remote_ip: Ipv4Address,
    remote_port: u16,
    timeout_spins: usize,
) -> Result<TcpStream, &'static str> {
    let local_port = TCP_LOCAL_PORT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let initial_seq = TCP_SEQ_COUNTER.fetch_add(0x1000, Ordering::Relaxed);

    let mut stream = TcpStream {
        remote_ip,
        remote_port,
        local_port,
        local_seq: initial_seq,
        remote_seq: 0,
        state: TcpState::SynSent,
    };

    // 1. Send SYN (with MSS option)
    stream.send_segment(TCP_FLAG_SYN, true, &[])?;

    // 2. Poll for matching SYN-ACK
    let mut rx_buf = [0u8; 1518];
    let mut syn_ack_received = false;

    for _ in 0..timeout_spins {
        if let Some(frame_len) = super::receive_packet(&mut rx_buf) {
            if let Ok((eth_header, eth_payload)) = EthernetHeader::parse(&rx_buf[..frame_len]) {
                if eth_header.ethertype == ETHERTYPE_IPV4 {
                    if let Ok((ip_header, ip_payload)) = Ipv4Header::parse(eth_payload) {
                        if ip_header.src_ip == remote_ip && ip_header.protocol == IP_PROTOCOL_TCP {
                            if let Ok((tcp_header, _)) = TcpHeader::parse(ip_payload) {
                                if tcp_header.src_port == remote_port && tcp_header.dst_port == local_port {
                                    if (tcp_header.flags & TCP_FLAG_RST) != 0 {
                                        return Err("Connection refused by peer (RST received)");
                                    }

                                    // Verify SYN and ACK flags and matching acknowledgement number
                                    let has_syn = (tcp_header.flags & TCP_FLAG_SYN) != 0;
                                    let has_ack = (tcp_header.flags & TCP_FLAG_ACK) != 0;

                                    if has_syn && has_ack && tcp_header.ack_num == initial_seq.wrapping_add(1) {
                                        stream.remote_seq = tcp_header.seq_num.wrapping_add(1);
                                        stream.local_seq = initial_seq.wrapping_add(1);
                                        syn_ack_received = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        core::hint::spin_loop();
    }

    if !syn_ack_received {
        return Err("TCP handshake timed out waiting for SYN-ACK");
    }

    // 3. Send final ACK to complete 3-way handshake
    stream.send_ack()?;
    stream.state = TcpState::Established;

    Ok(stream)
}
