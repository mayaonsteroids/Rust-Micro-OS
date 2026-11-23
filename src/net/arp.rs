//! Address Resolution Protocol (ARP) for IPv4 over Ethernet (RFC 826).
//!
//! Handles ARP packet parsing, request/reply generation, and a bounded ARP cache table.

use spin::Mutex;
use super::ethernet::MacAddress;

pub const ARP_HARDWARE_ETHERNET: u16 = 1;
pub const ARP_PROTOCOL_IPV4: u16 = 0x0800;
pub const ARP_HW_ADDR_LEN: u8 = 6;
pub const ARP_PROTO_ADDR_LEN: u8 = 4;

pub const ARP_OPCODE_REQUEST: u16 = 1;
pub const ARP_OPCODE_REPLY: u16 = 2;

pub const ARP_PACKET_LEN: usize = 28;
pub const ARP_CACHE_CAPACITY: usize = 16;

/// Parsed ARP Packet structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArpPacket {
    pub hardware_type: u16,
    pub protocol_type: u16,
    pub hardware_len: u8,
    pub protocol_len: u8,
    pub opcode: u16,
    pub sender_mac: MacAddress,
    pub sender_ip: [u8; 4],
    pub target_mac: MacAddress,
    pub target_ip: [u8; 4],
}

impl ArpPacket {
    /// Parses and validates an ARP packet from raw bytes.
    pub fn parse(raw: &[u8]) -> Result<Self, &'static str> {
        if raw.len() < ARP_PACKET_LEN {
            return Err("Packet shorter than 28-byte ARP header");
        }

        let hardware_type = ((raw[0] as u16) << 8) | (raw[1] as u16);
        let protocol_type = ((raw[2] as u16) << 8) | (raw[3] as u16);
        let hardware_len = raw[4];
        let protocol_len = raw[5];
        let opcode = ((raw[6] as u16) << 8) | (raw[7] as u16);

        if hardware_type != ARP_HARDWARE_ETHERNET {
            return Err("Unsupported ARP hardware type (expected Ethernet 1)");
        }
        if protocol_type != ARP_PROTOCOL_IPV4 {
            return Err("Unsupported ARP protocol type (expected IPv4 0x0800)");
        }
        if hardware_len != ARP_HW_ADDR_LEN {
            return Err("Invalid ARP hardware address length (expected 6)");
        }
        if protocol_len != ARP_PROTO_ADDR_LEN {
            return Err("Invalid ARP protocol address length (expected 4)");
        }
        if opcode != ARP_OPCODE_REQUEST && opcode != ARP_OPCODE_REPLY {
            return Err("Unknown ARP opcode (expected 1 for Request or 2 for Reply)");
        }

        let mut sender_mac = [0u8; 6];
        let mut sender_ip = [0u8; 4];
        let mut target_mac = [0u8; 6];
        let mut target_ip = [0u8; 4];

        sender_mac.copy_from_slice(&raw[8..14]);
        sender_ip.copy_from_slice(&raw[14..18]);
        target_mac.copy_from_slice(&raw[18..24]);
        target_ip.copy_from_slice(&raw[24..28]);

        Ok(Self {
            hardware_type,
            protocol_type,
            hardware_len,
            protocol_len,
            opcode,
            sender_mac,
            sender_ip,
            target_mac,
            target_ip,
        })
    }

    /// Serializes an ARP Request into `out_buf`.
    pub fn build_request(
        sender_mac: MacAddress,
        sender_ip: [u8; 4],
        target_ip: [u8; 4],
        out_buf: &mut [u8],
    ) -> Result<usize, &'static str> {
        if out_buf.len() < ARP_PACKET_LEN {
            return Err("Output buffer too small for ARP packet");
        }

        // Hardware type (Ethernet = 1)
        out_buf[0] = 0x00;
        out_buf[1] = 0x01;
        // Protocol type (IPv4 = 0x0800)
        out_buf[2] = 0x08;
        out_buf[3] = 0x00;
        // Hardware size (6), Protocol size (4)
        out_buf[4] = 0x06;
        out_buf[5] = 0x04;
        // Opcode (Request = 1)
        out_buf[6] = 0x00;
        out_buf[7] = 0x01;
        // Sender MAC
        out_buf[8..14].copy_from_slice(&sender_mac);
        // Sender IP
        out_buf[14..18].copy_from_slice(&sender_ip);
        // Target MAC (00:00:00:00:00:00 in request)
        out_buf[18..24].copy_from_slice(&[0x00; 6]);
        // Target IP
        out_buf[24..28].copy_from_slice(&target_ip);

        Ok(ARP_PACKET_LEN)
    }

    /// Serializes an ARP Reply into `out_buf`.
    pub fn build_reply(
        sender_mac: MacAddress,
        sender_ip: [u8; 4],
        target_mac: MacAddress,
        target_ip: [u8; 4],
        out_buf: &mut [u8],
    ) -> Result<usize, &'static str> {
        if out_buf.len() < ARP_PACKET_LEN {
            return Err("Output buffer too small for ARP packet");
        }

        // Hardware type (Ethernet = 1)
        out_buf[0] = 0x00;
        out_buf[1] = 0x01;
        // Protocol type (IPv4 = 0x0800)
        out_buf[2] = 0x08;
        out_buf[3] = 0x00;
        // Hardware size (6), Protocol size (4)
        out_buf[4] = 0x06;
        out_buf[5] = 0x04;
        // Opcode (Reply = 2)
        out_buf[6] = 0x00;
        out_buf[7] = 0x02;
        // Sender MAC
        out_buf[8..14].copy_from_slice(&sender_mac);
        // Sender IP
        out_buf[14..18].copy_from_slice(&sender_ip);
        // Target MAC
        out_buf[18..24].copy_from_slice(&target_mac);
        // Target IP
        out_buf[24..28].copy_from_slice(&target_ip);

        Ok(ARP_PACKET_LEN)
    }
}

/// Single bounded entry in the ARP cache.
#[derive(Debug, Clone, Copy)]
pub struct ArpEntry {
    pub ip: [u8; 4],
    pub mac: MacAddress,
    pub valid: bool,
}

impl ArpEntry {
    pub const fn empty() -> Self {
        Self {
            ip: [0; 4],
            mac: [0; 6],
            valid: false,
        }
    }
}

/// Bounded ARP Cache table.
pub struct ArpCache {
    pub entries: [ArpEntry; ARP_CACHE_CAPACITY],
    pub next_idx: usize,
}

impl ArpCache {
    pub const fn new() -> Self {
        Self {
            entries: [ArpEntry::empty(); ARP_CACHE_CAPACITY],
            next_idx: 0,
        }
    }

    /// Looks up an IPv4 address in the ARP cache.
    pub fn lookup(&self, ip: &[u8; 4]) -> Option<MacAddress> {
        for entry in self.entries.iter() {
            if entry.valid && entry.ip == *ip {
                return Some(entry.mac);
            }
        }
        None
    }

    /// Inserts or updates an IP-to-MAC mapping in the ARP cache.
    pub fn insert(&mut self, ip: [u8; 4], mac: MacAddress) {
        // If entry exists, update MAC
        for entry in self.entries.iter_mut() {
            if entry.valid && entry.ip == ip {
                entry.mac = mac;
                return;
            }
        }

        // Otherwise find first invalid slot
        for entry in self.entries.iter_mut() {
            if !entry.valid {
                entry.ip = ip;
                entry.mac = mac;
                entry.valid = true;
                return;
            }
        }

        // If table is completely full, use round-robin eviction
        let idx = self.next_idx;
        self.entries[idx].ip = ip;
        self.entries[idx].mac = mac;
        self.entries[idx].valid = true;
        self.next_idx = (self.next_idx + 1) % ARP_CACHE_CAPACITY;
    }

    /// Clears all entries from the ARP cache.
    pub fn clear(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.valid = false;
        }
        self.next_idx = 0;
    }
}

/// Global synchronized ARP cache.
pub static ARP_CACHE: Mutex<ArpCache> = Mutex::new(ArpCache::new());
