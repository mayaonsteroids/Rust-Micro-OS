//! Internet Control Message Protocol (ICMP) for IPv4 (RFC 792).
//!
//! Provides ICMP Echo Request and Echo Reply handling for genuine `ping` functionality.

pub const ICMP_TYPE_ECHO_REPLY: u8 = 0;
pub const ICMP_TYPE_ECHO_REQUEST: u8 = 8;
pub const ICMP_CODE_DEFAULT: u8 = 0;
pub const ICMP_HEADER_LEN: usize = 8;

/// Parsed ICMP Echo Header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IcmpEchoHeader {
    pub icmp_type: u8,
    pub code: u8,
    pub checksum: u16,
    pub identifier: u16,
    pub sequence_number: u16,
}

impl IcmpEchoHeader {
    /// Calculates the 16-bit one's complement checksum for an ICMP message.
    pub fn calculate_checksum(msg: &[u8]) -> u16 {
        let mut sum: u32 = 0;
        let words = msg.len() / 2;

        for i in 0..words {
            if i == 1 {
                // Bytes 2..4 are the checksum field; skip
                continue;
            }
            let word = ((msg[i * 2] as u32) << 8) | (msg[i * 2 + 1] as u32);
            sum = sum.wrapping_add(word);
        }

        if (msg.len() % 2) != 0 {
            let last_byte = msg[msg.len() - 1];
            sum = sum.wrapping_add((last_byte as u32) << 8);
        }

        while (sum >> 16) != 0 {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }

        !sum as u16
    }

    /// Parses an ICMP Echo message from raw bytes.
    pub fn parse<'a>(raw: &'a [u8]) -> Result<(Self, &'a [u8]), &'static str> {
        if raw.len() < ICMP_HEADER_LEN {
            return Err("Packet shorter than 8-byte ICMP header");
        }

        let icmp_type = raw[0];
        let code = raw[1];
        let checksum = ((raw[2] as u16) << 8) | (raw[3] as u16);
        let identifier = ((raw[4] as u16) << 8) | (raw[5] as u16);
        let sequence_number = ((raw[6] as u16) << 8) | (raw[7] as u16);

        let header = Self {
            icmp_type,
            code,
            checksum,
            identifier,
            sequence_number,
        };

        let payload = &raw[ICMP_HEADER_LEN..];
        Ok((header, payload))
    }

    /// Serializes an ICMP Echo message into `out_buf` with valid checksum.
    pub fn build_message(
        icmp_type: u8,
        code: u8,
        identifier: u16,
        sequence_number: u16,
        payload: &[u8],
        out_buf: &mut [u8],
    ) -> Result<usize, &'static str> {
        let total_nominal = ICMP_HEADER_LEN + payload.len();
        if out_buf.len() < total_nominal {
            return Err("Output buffer too small for ICMP message");
        }

        out_buf[0] = icmp_type;
        out_buf[1] = code;
        out_buf[2] = 0x00; // Checksum placeholder
        out_buf[3] = 0x00;
        out_buf[4] = (identifier >> 8) as u8;
        out_buf[5] = (identifier & 0xFF) as u8;
        out_buf[6] = (sequence_number >> 8) as u8;
        out_buf[7] = (sequence_number & 0xFF) as u8;

        out_buf[ICMP_HEADER_LEN..total_nominal].copy_from_slice(payload);

        let checksum = Self::calculate_checksum(&out_buf[..total_nominal]);
        out_buf[2] = (checksum >> 8) as u8;
        out_buf[3] = (checksum & 0xFF) as u8;

        Ok(total_nominal)
    }
}
