//! Ethernet II Frame parsing and construction.
//!
//! Conforms to IEEE 802.3 / Ethernet II framing standards.
//! Provides bounds-checked parsing, EtherType filtering, and frame serialization.

/// 6-byte IEEE 802.3 MAC Address.
pub type MacAddress = [u8; 6];

/// Standard Ethernet broadcast address (FF:FF:FF:FF:FF:FF).
pub const BROADCAST_MAC: MacAddress = [0xFF; 6];

/// Ethernet II EtherTypes.
pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;

/// Minimum Ethernet frame size without FCS (header + minimum payload).
pub const ETHERNET_MIN_FRAME_LEN: usize = 60;
/// Standard Ethernet header length (Dest MAC [6] + Src MAC [6] + EtherType [2]).
pub const ETHERNET_HEADER_LEN: usize = 14;
/// Maximum standard Ethernet frame size without FCS (1500 byte MTU + 14 byte header).
pub const ETHERNET_MAX_FRAME_LEN: usize = 1514;

/// Parsed Ethernet II header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EthernetHeader {
    pub dest_mac: MacAddress,
    pub src_mac: MacAddress,
    pub ethertype: u16,
}

impl EthernetHeader {
    /// Parses an Ethernet II header from raw packet bytes.
    ///
    /// Performs strict bounds checking: returns an error if `raw` is shorter than 14 bytes.
    pub fn parse(raw: &[u8]) -> Result<(Self, &[u8]), &'static str> {
        if raw.len() < ETHERNET_HEADER_LEN {
            return Err("Packet frame shorter than 14-byte Ethernet header");
        }

        let mut dest_mac = [0u8; 6];
        let mut src_mac = [0u8; 6];
        dest_mac.copy_from_slice(&raw[0..6]);
        src_mac.copy_from_slice(&raw[6..12]);
        let ethertype = ((raw[12] as u16) << 8) | (raw[13] as u16);

        let header = Self {
            dest_mac,
            src_mac,
            ethertype,
        };

        let payload = &raw[ETHERNET_HEADER_LEN..];
        Ok((header, payload))
    }

    /// Constructs and serializes an Ethernet II frame into `out_buf`.
    ///
    /// Pads frames shorter than 60 bytes with zeros.
    /// Returns the total serialized frame length in bytes.
    pub fn build_frame(
        dest_mac: MacAddress,
        src_mac: MacAddress,
        ethertype: u16,
        payload: &[u8],
        out_buf: &mut [u8],
    ) -> Result<usize, &'static str> {
        let payload_len = payload.len();
        let total_nominal = ETHERNET_HEADER_LEN + payload_len;
        let total_len = if total_nominal < ETHERNET_MIN_FRAME_LEN {
            ETHERNET_MIN_FRAME_LEN
        } else {
            total_nominal
        };

        if out_buf.len() < total_len {
            return Err("Output buffer too small for constructed Ethernet frame");
        }

        if total_nominal > ETHERNET_MAX_FRAME_LEN {
            return Err("Payload exceeds maximum Ethernet MTU (1500 bytes)");
        }

        // Destination MAC
        out_buf[0..6].copy_from_slice(&dest_mac);
        // Source MAC
        out_buf[6..12].copy_from_slice(&src_mac);
        // EtherType
        out_buf[12] = (ethertype >> 8) as u8;
        out_buf[13] = (ethertype & 0xFF) as u8;

        // Copy payload
        out_buf[ETHERNET_HEADER_LEN..ETHERNET_HEADER_LEN + payload_len].copy_from_slice(payload);

        // Zero-pad to 60 bytes minimum if necessary
        if total_nominal < ETHERNET_MIN_FRAME_LEN {
            for b in &mut out_buf[total_nominal..ETHERNET_MIN_FRAME_LEN] {
                *b = 0;
            }
        }

        Ok(total_len)
    }
}
