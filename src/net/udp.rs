//! User Datagram Protocol (UDP) packet handling (RFC 768).
//!
//! Provides UDP header parsing, serialization, length validation, and IPv4 pseudo-header checksumming.

use super::ipv4::Ipv4Address;

pub const UDP_HEADER_LEN: usize = 8;
pub const MAX_UDP_PAYLOAD_LEN: usize = 1472; // Standard Ethernet MTU 1500 - 20 (IP) - 8 (UDP)

/// Parsed UDP Datagram Header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UdpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub checksum: u16,
}

impl UdpHeader {
    /// Calculates the RFC 768 16-bit one's complement checksum using the IPv4 pseudo-header.
    pub fn calculate_checksum(
        src_ip: &Ipv4Address,
        dst_ip: &Ipv4Address,
        udp_packet: &[u8],
    ) -> u16 {
        let mut sum: u32 = 0;

        // IPv4 Pseudo-Header:
        // Source IP (2 words)
        sum = sum.wrapping_add(((src_ip[0] as u32) << 8) | (src_ip[1] as u32));
        sum = sum.wrapping_add(((src_ip[2] as u32) << 8) | (src_ip[3] as u32));
        // Destination IP (2 words)
        sum = sum.wrapping_add(((dst_ip[0] as u32) << 8) | (dst_ip[1] as u32));
        sum = sum.wrapping_add(((dst_ip[2] as u32) << 8) | (dst_ip[3] as u32));
        // Zero (8 bits) + Protocol 17 (8 bits)
        sum = sum.wrapping_add(17u32);
        // UDP Length (16 bits)
        let udp_len = udp_packet.len() as u32;
        sum = sum.wrapping_add(udp_len);

        // UDP Header + Data (with checksum field at bytes 6..8 skipped/treated as 0)
        let words = udp_packet.len() / 2;
        for i in 0..words {
            if i == 3 {
                // Checksum field; skip
                continue;
            }
            let word = ((udp_packet[i * 2] as u32) << 8) | (udp_packet[i * 2 + 1] as u32);
            sum = sum.wrapping_add(word);
        }

        // Odd trailing byte if any
        if (udp_packet.len() % 2) != 0 {
            let last_byte = udp_packet[udp_packet.len() - 1];
            sum = sum.wrapping_add((last_byte as u32) << 8);
        }

        // Fold carry bits
        while (sum >> 16) != 0 {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }

        let checksum = !sum as u16;
        if checksum == 0 {
            0xFFFF
        } else {
            checksum
        }
    }

    /// Verifies the checksum of a UDP packet against its IPv4 pseudo-header.
    pub fn verify_checksum(
        src_ip: &Ipv4Address,
        dst_ip: &Ipv4Address,
        udp_packet: &[u8],
    ) -> bool {
        if udp_packet.len() < UDP_HEADER_LEN {
            return false;
        }
        let stored_checksum = ((udp_packet[6] as u16) << 8) | (udp_packet[7] as u16);
        if stored_checksum == 0 {
            // Checksum was not calculated by sender (optional in IPv4)
            return true;
        }
        let computed = Self::calculate_checksum(src_ip, dst_ip, udp_packet);
        stored_checksum == computed
    }

    /// Parses a UDP datagram from raw bytes.
    ///
    /// Validates datagram length and payload boundaries.
    pub fn parse<'a>(raw: &'a [u8]) -> Result<(Self, &'a [u8]), &'static str> {
        if raw.len() < UDP_HEADER_LEN {
            return Err("Packet shorter than 8-byte UDP header");
        }

        let src_port = ((raw[0] as u16) << 8) | (raw[1] as u16);
        let dst_port = ((raw[2] as u16) << 8) | (raw[3] as u16);
        let length = ((raw[4] as u16) << 8) | (raw[5] as u16);
        let checksum = ((raw[6] as u16) << 8) | (raw[7] as u16);

        if (length as usize) < UDP_HEADER_LEN {
            return Err("UDP length field is smaller than header size (8 bytes)");
        }
        if raw.len() < (length as usize) {
            return Err("Truncated UDP packet: buffer length is smaller than UDP length field");
        }

        let header = Self {
            src_port,
            dst_port,
            length,
            checksum,
        };

        let payload_len = (length as usize) - UDP_HEADER_LEN;
        let payload = &raw[UDP_HEADER_LEN..UDP_HEADER_LEN + payload_len];

        Ok((header, payload))
    }

    /// Serializes a UDP datagram into `out_buf` with valid IPv4 pseudo-header checksum.
    pub fn build_packet(
        src_ip: &Ipv4Address,
        dst_ip: &Ipv4Address,
        src_port: u16,
        dst_port: u16,
        payload: &[u8],
        out_buf: &mut [u8],
    ) -> Result<usize, &'static str> {
        let total_nominal = UDP_HEADER_LEN + payload.len();
        if total_nominal > 0xFFFF {
            return Err("UDP packet size exceeds 65535 bytes");
        }
        if out_buf.len() < total_nominal {
            return Err("Output buffer too small for UDP packet");
        }

        out_buf[0] = (src_port >> 8) as u8;
        out_buf[1] = (src_port & 0xFF) as u8;
        out_buf[2] = (dst_port >> 8) as u8;
        out_buf[3] = (dst_port & 0xFF) as u8;
        out_buf[4] = (total_nominal >> 8) as u8;
        out_buf[5] = (total_nominal & 0xFF) as u8;
        out_buf[6] = 0x00; // Checksum placeholder
        out_buf[7] = 0x00;

        // Copy payload
        out_buf[UDP_HEADER_LEN..total_nominal].copy_from_slice(payload);

        // Compute and store checksum
        let checksum = Self::calculate_checksum(src_ip, dst_ip, &out_buf[..total_nominal]);
        out_buf[6] = (checksum >> 8) as u8;
        out_buf[7] = (checksum & 0xFF) as u8;

        Ok(total_nominal)
    }
}
