//! Domain Name System (DNS) Client (RFC 1035).
//!
//! Provides A-record query generation, response parsing with label decompression,
//! and a bounded in-memory DNS resolution cache.

use spin::Mutex;
use super::ipv4::Ipv4Address;

pub const DNS_PORT: u16 = 53;
pub const DNS_FLAG_RECURSION_DESIRED: u16 = 0x0100;
pub const DNS_TYPE_A: u16 = 1;
pub const DNS_CLASS_IN: u16 = 1;

pub const DNS_CACHE_CAPACITY: usize = 8;
pub const MAX_DOMAIN_LEN: usize = 64;

/// Encodes a domain name string into DNS label format.
///
/// Example: "example.com" -> `[7, b'e', b'x', b'a', b'm', b'p', b'l', b'e', 3, b'c', b'o', b'm', 0]`
pub fn encode_domain_name(domain: &str, out_buf: &mut [u8]) -> Result<usize, &'static str> {
    if domain.is_empty() || domain.len() > MAX_DOMAIN_LEN {
        return Err("Domain name length invalid (1-64 characters required)");
    }

    let mut out_idx = 0;
    for label in domain.split('.') {
        if label.is_empty() {
            continue;
        }
        let len = label.len();
        if len > 63 {
            return Err("DNS label exceeds 63-character limit");
        }
        if out_idx + 1 + len >= out_buf.len() {
            return Err("Buffer too small for encoded domain name");
        }
        out_buf[out_idx] = len as u8;
        out_idx += 1;
        out_buf[out_idx..out_idx + len].copy_from_slice(label.as_bytes());
        out_idx += len;
    }

    if out_idx >= out_buf.len() {
        return Err("Buffer too small for terminating zero label");
    }
    out_buf[out_idx] = 0; // Terminating zero length octet
    out_idx += 1;

    Ok(out_idx)
}

/// Constructs a standard DNS A-record query packet into `out_buf`.
pub fn build_dns_query(domain: &str, xid: u16, out_buf: &mut [u8]) -> Result<usize, &'static str> {
    if out_buf.len() < 12 + MAX_DOMAIN_LEN + 4 {
        return Err("Output buffer too small for DNS query");
    }

    // Header (12 bytes):
    // ID (2 bytes)
    out_buf[0] = (xid >> 8) as u8;
    out_buf[1] = (xid & 0xFF) as u8;
    // Flags (2 bytes): Standard query with Recursion Desired (0x0100)
    out_buf[2] = 0x01;
    out_buf[3] = 0x00;
    // QDCOUNT: 1 question
    out_buf[4] = 0x00;
    out_buf[5] = 0x01;
    // ANCOUNT: 0
    out_buf[6] = 0x00;
    out_buf[7] = 0x00;
    // NSCOUNT: 0
    out_buf[8] = 0x00;
    out_buf[9] = 0x00;
    // ARCOUNT: 0
    out_buf[10] = 0x00;
    out_buf[11] = 0x00;

    let mut idx = 12;

    // Question Section: QNAME
    let name_len = encode_domain_name(domain, &mut out_buf[idx..])?;
    idx += name_len;

    // QTYPE: A (1)
    out_buf[idx] = 0x00;
    out_buf[idx + 1] = 0x01;
    // QCLASS: IN (1)
    out_buf[idx + 2] = 0x00;
    out_buf[idx + 3] = 0x01;
    idx += 4;

    Ok(idx)
}

/// Safely advances past a DNS name (handling compression pointers) with recursion/hop bounds.
fn skip_dns_name(raw: &[u8], mut offset: usize) -> Result<usize, &'static str> {
    let mut hops = 0;
    while offset < raw.len() {
        let len = raw[offset];
        if len == 0 {
            return Ok(offset + 1);
        }
        if (len & 0xC0) == 0xC0 {
            // Pointer (2 bytes total)
            if offset + 1 >= raw.len() {
                return Err("Truncated DNS compression pointer");
            }
            return Ok(offset + 2);
        } else {
            // Standard label
            offset += 1 + (len as usize);
            hops += 1;
            if hops > 64 {
                return Err("Malformed DNS name exceeding 64 labels");
            }
        }
    }
    Err("Unexpected end of packet while parsing DNS name")
}

/// Parses a DNS response packet and extracts the first matching IPv4 A-record.
pub fn parse_dns_response(raw: &[u8], expected_xid: u16) -> Result<Ipv4Address, &'static str> {
    if raw.len() < 12 {
        return Err("Packet shorter than 12-byte DNS header");
    }

    let xid = ((raw[0] as u16) << 8) | (raw[1] as u16);
    if xid != expected_xid {
        return Err("DNS transaction ID mismatch");
    }

    let flags = ((raw[2] as u16) << 8) | (raw[3] as u16);
    let is_response = (flags & 0x8000) != 0;
    if !is_response {
        return Err("Received packet is not a DNS response");
    }

    let rcode = flags & 0x000F;
    if rcode == 3 {
        return Err("DNS name does not exist (NXDOMAIN)");
    } else if rcode != 0 {
        return Err("DNS server returned an error code");
    }

    let qdcount = ((raw[4] as u16) << 8) | (raw[5] as u16);
    let ancount = ((raw[6] as u16) << 8) | (raw[7] as u16);

    if ancount == 0 {
        return Err("DNS response contains zero answer records");
    }

    let mut offset = 12;

    // Skip question records
    for _ in 0..qdcount {
        offset = skip_dns_name(raw, offset)?;
        if offset + 4 > raw.len() {
            return Err("Truncated question record in DNS response");
        }
        offset += 4; // Skip QTYPE and QCLASS
    }

    // Parse answer records
    for _ in 0..ancount {
        offset = skip_dns_name(raw, offset)?;
        if offset + 10 > raw.len() {
            return Err("Truncated resource record header in DNS answer");
        }

        let rtype = ((raw[offset] as u16) << 8) | (raw[offset + 1] as u16);
        let rclass = ((raw[offset + 2] as u16) << 8) | (raw[offset + 3] as u16);
        // let ttl = ... bytes 4..8
        let rdlength = ((raw[offset + 8] as usize) << 8) | (raw[offset + 9] as usize);
        offset += 10;

        if offset + rdlength > raw.len() {
            return Err("DNS resource data length exceeds packet boundary");
        }

        // Check for IPv4 A-record in Internet class
        if rtype == DNS_TYPE_A && rclass == DNS_CLASS_IN && rdlength == 4 {
            let mut ip = [0u8; 4];
            ip.copy_from_slice(&raw[offset..offset + 4]);
            return Ok(ip);
        }

        offset += rdlength;
    }

    Err("No IPv4 A-record found in DNS answer section")
}

/// Single bounded entry in the DNS resolution cache.
#[derive(Debug, Clone, Copy)]
pub struct DnsCacheEntry {
    pub domain: [u8; MAX_DOMAIN_LEN],
    pub domain_len: usize,
    pub ip: Ipv4Address,
    pub valid: bool,
}

impl DnsCacheEntry {
    pub const fn empty() -> Self {
        Self {
            domain: [0; MAX_DOMAIN_LEN],
            domain_len: 0,
            ip: [0; 4],
            valid: false,
        }
    }
}

/// Bounded in-memory DNS cache table.
pub struct DnsCache {
    pub entries: [DnsCacheEntry; DNS_CACHE_CAPACITY],
    pub next_idx: usize,
}

impl DnsCache {
    pub const fn new() -> Self {
        Self {
            entries: [DnsCacheEntry::empty(); DNS_CACHE_CAPACITY],
            next_idx: 0,
        }
    }

    /// Looks up a domain name in the DNS cache.
    pub fn lookup(&self, domain: &str) -> Option<Ipv4Address> {
        let domain_bytes = domain.as_bytes();
        for entry in self.entries.iter() {
            if entry.valid
                && entry.domain_len == domain_bytes.len()
                && &entry.domain[..entry.domain_len] == domain_bytes
            {
                return Some(entry.ip);
            }
        }
        None
    }

    /// Inserts or updates a domain resolution entry in the DNS cache.
    pub fn insert(&mut self, domain: &str, ip: Ipv4Address) {
        let bytes = domain.as_bytes();
        let copy_len = core::cmp::min(bytes.len(), MAX_DOMAIN_LEN);

        // Update if already cached
        for entry in self.entries.iter_mut() {
            if entry.valid
                && entry.domain_len == copy_len
                && &entry.domain[..entry.domain_len] == &bytes[..copy_len]
            {
                entry.ip = ip;
                return;
            }
        }

        // Insert into first vacant slot
        for entry in self.entries.iter_mut() {
            if !entry.valid {
                entry.domain[..copy_len].copy_from_slice(&bytes[..copy_len]);
                entry.domain_len = copy_len;
                entry.ip = ip;
                entry.valid = true;
                return;
            }
        }

        // Round-robin eviction when full
        let idx = self.next_idx;
        self.entries[idx].domain[..copy_len].copy_from_slice(&bytes[..copy_len]);
        self.entries[idx].domain_len = copy_len;
        self.entries[idx].ip = ip;
        self.entries[idx].valid = true;
        self.next_idx = (self.next_idx + 1) % DNS_CACHE_CAPACITY;
    }

    /// Clears the DNS cache.
    pub fn clear(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.valid = false;
        }
        self.next_idx = 0;
    }
}

pub static DNS_CACHE: Mutex<DnsCache> = Mutex::new(DnsCache::new());
