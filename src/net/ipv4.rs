//! Internet Protocol version 4 (IPv4) packet handling (RFC 791).
//!
//! Provides header parsing, serialization, length/version validation, and RFC 791 checksumming.

pub const IPV4_VERSION: u8 = 4;
pub const IPV4_MIN_IHL: u8 = 5;
pub const IPV4_MIN_HEADER_LEN: usize = 20;

pub const IP_PROTOCOL_ICMP: u8 = 1;
pub const IP_PROTOCOL_TCP: u8 = 6;
pub const IP_PROTOCOL_UDP: u8 = 17;

/// 4-byte IPv4 Address.
pub type Ipv4Address = [u8; 4];

/// Standard IPv4 loopback (127.0.0.1) and broadcast (255.255.255.255).
pub const IPV4_BROADCAST: Ipv4Address = [255, 255, 255, 255];
pub const IPV4_LOOPBACK: Ipv4Address = [127, 0, 0, 1];

/// Parses a string representation of an IPv4 address (e.g. "192.168.1.1").
pub fn parse_ipv4_addr(s: &str) -> Option<Ipv4Address> {
    let mut parts = s.split('.');
    let a: u8 = parse_u8_dec(parts.next()?)?;
    let b: u8 = parse_u8_dec(parts.next()?)?;
    let c: u8 = parse_u8_dec(parts.next()?)?;
    let d: u8 = parse_u8_dec(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    Some([a, b, c, d])
}

fn parse_u8_dec(s: &str) -> Option<u8> {
    let mut val: u16 = 0;
    if s.is_empty() || s.len() > 3 {
        return None;
    }
    for b in s.bytes() {
        if !b.is_ascii_digit() {
            return None;
        }
        val = val * 10 + (b - b'0') as u16;
    }
    if val <= 255 {
        Some(val as u8)
    } else {
        None
    }
}

/// Parsed IPv4 Header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv4Header {
    pub version: u8,
    pub ihl: u8, // Internet Header Length in 32-bit words (>= 5)
    pub dscp_ecn: u8,
    pub total_length: u16,
    pub identification: u16,
    pub flags_fragment: u16,
    pub ttl: u8,
    pub protocol: u8,
    pub checksum: u16,
    pub src_ip: Ipv4Address,
    pub dst_ip: Ipv4Address,
}

impl Ipv4Header {
    /// Returns the length of the IPv4 header in bytes (`ihl * 4`).
    #[inline(always)]
    pub fn header_len(&self) -> usize {
        (self.ihl as usize) * 4
    }

    /// Calculates the RFC 791 16-bit one's complement checksum for an IPv4 header.
    ///
    /// Computes the sum with the checksum field (bytes 10..12) treated as zero.
    pub fn calculate_checksum(header_bytes: &[u8]) -> u16 {
        let mut sum: u32 = 0;
        let words = header_bytes.len() / 2;

        for i in 0..words {
            if i == 5 {
                // Bytes 10..12 are the checksum field; skip them (treat as zero)
                continue;
            }
            let word = ((header_bytes[i * 2] as u32) << 8) | (header_bytes[i * 2 + 1] as u32);
            sum = sum.wrapping_add(word);
        }

        // Fold carry bits into 16 bits
        while (sum >> 16) != 0 {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }

        !sum as u16
    }

    /// Verifies the checksum of a raw IPv4 header.
    pub fn verify_checksum(header_bytes: &[u8]) -> bool {
        if header_bytes.len() < IPV4_MIN_HEADER_LEN {
            return false;
        }
        let stored_checksum = ((header_bytes[10] as u16) << 8) | (header_bytes[11] as u16);
        let computed = Self::calculate_checksum(header_bytes);
        stored_checksum == computed
    }

    /// Parses an IPv4 packet from raw bytes.
    ///
    /// Validates version (4), IHL (>= 5), total length, and header checksum.
    /// Returns the parsed header and the slice corresponding to the payload.
    pub fn parse(raw: &[u8]) -> Result<(Self, &[u8]), &'static str> {
        if raw.len() < IPV4_MIN_HEADER_LEN {
            return Err("Packet shorter than 20-byte IPv4 minimum header");
        }

        let ver_ihl = raw[0];
        let version = ver_ihl >> 4;
        let ihl = ver_ihl & 0x0F;

        if version != IPV4_VERSION {
            return Err("Unsupported IP version (expected IPv4)");
        }
        if ihl < IPV4_MIN_IHL {
            return Err("Invalid IPv4 IHL (must be >= 5)");
        }

        let header_len = (ihl as usize) * 4;
        if raw.len() < header_len {
            return Err("Packet shorter than IPv4 header length indicated by IHL");
        }

        let total_length = ((raw[2] as u16) << 8) | (raw[3] as u16);
        if (total_length as usize) < header_len {
            return Err("IPv4 total length cannot be smaller than header length");
        }
        if raw.len() < (total_length as usize) {
            return Err("Truncated IPv4 packet: packet bytes less than total length");
        }

        // Verify checksum
        if !Self::verify_checksum(&raw[..header_len]) {
            return Err("IPv4 header checksum mismatch");
        }

        let dscp_ecn = raw[1];
        let identification = ((raw[4] as u16) << 8) | (raw[5] as u16);
        let flags_fragment = ((raw[6] as u16) << 8) | (raw[7] as u16);
        let ttl = raw[8];
        let protocol = raw[9];
        let checksum = ((raw[10] as u16) << 8) | (raw[11] as u16);

        let mut src_ip = [0u8; 4];
        let mut dst_ip = [0u8; 4];
        src_ip.copy_from_slice(&raw[12..16]);
        dst_ip.copy_from_slice(&raw[16..20]);

        let header = Self {
            version,
            ihl,
            dscp_ecn,
            total_length,
            identification,
            flags_fragment,
            ttl,
            protocol,
            checksum,
            src_ip,
            dst_ip,
        };

        // Extract the valid payload slice based on total_length
        let payload_end = total_length as usize;
        let payload = &raw[header_len..payload_end];

        Ok((header, payload))
    }

    /// Serializes an IPv4 packet with standard 20-byte header (IHL = 5) into `out_buf`.
    ///
    /// Computes and writes the valid RFC 791 header checksum.
    /// Returns the total packet length in bytes.
    pub fn build_packet(
        src_ip: Ipv4Address,
        dst_ip: Ipv4Address,
        protocol: u8,
        identification: u16,
        payload: &[u8],
        out_buf: &mut [u8],
    ) -> Result<usize, &'static str> {
        let total_nominal = IPV4_MIN_HEADER_LEN + payload.len();
        if total_nominal > 0xFFFF {
            return Err("IPv4 total length exceeds 65535 bytes");
        }
        if out_buf.len() < total_nominal {
            return Err("Output buffer too small for IPv4 packet");
        }

        out_buf[0] = (IPV4_VERSION << 4) | IPV4_MIN_IHL; // Version 4, IHL 5 (20 bytes)
        out_buf[1] = 0x00; // DSCP / ECN
        out_buf[2] = (total_nominal >> 8) as u8;
        out_buf[3] = (total_nominal & 0xFF) as u8;
        out_buf[4] = (identification >> 8) as u8;
        out_buf[5] = (identification & 0xFF) as u8;
        out_buf[6] = 0x40; // Flags: Don't Fragment (DF bit set), Fragment offset 0
        out_buf[7] = 0x00;
        out_buf[8] = 64; // TTL: 64 hops
        out_buf[9] = protocol;
        out_buf[10] = 0x00; // Checksum placeholder
        out_buf[11] = 0x00;
        out_buf[12..16].copy_from_slice(&src_ip);
        out_buf[16..20].copy_from_slice(&dst_ip);

        // Copy payload
        out_buf[IPV4_MIN_HEADER_LEN..total_nominal].copy_from_slice(payload);

        // Calculate and insert header checksum
        let checksum = Self::calculate_checksum(&out_buf[..IPV4_MIN_HEADER_LEN]);
        out_buf[10] = (checksum >> 8) as u8;
        out_buf[11] = (checksum & 0xFF) as u8;

        Ok(total_nominal)
    }
}
