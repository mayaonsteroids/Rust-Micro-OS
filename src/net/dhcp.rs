//! Dynamic Host Configuration Protocol (DHCP) Client (RFC 2131 & RFC 2132).
//!
//! Implements automated IPv4 address configuration via DHCPDISCOVER, DHCPOFFER,
//! DHCPREQUEST, and DHCPACK over UDP ports 67 (server) and 68 (client).

use super::ethernet::MacAddress;
use super::ipv4::Ipv4Address;

pub const DHCP_SERVER_PORT: u16 = 67;
pub const DHCP_CLIENT_PORT: u16 = 68;

pub const DHCP_OP_REQUEST: u8 = 1;
pub const DHCP_OP_REPLY: u8 = 2;
pub const DHCP_HTYPE_ETHERNET: u8 = 1;
pub const DHCP_HLEN_ETHERNET: u8 = 6;

pub const DHCP_MAGIC_COOKIE: [u8; 4] = [0x63, 0x82, 0x53, 0x63];

// DHCP Message Types (Option 53)
pub const DHCP_MSG_DISCOVER: u8 = 1;
pub const DHCP_MSG_OFFER: u8 = 2;
pub const DHCP_MSG_REQUEST: u8 = 3;
pub const DHCP_MSG_DECLINE: u8 = 4;
pub const DHCP_MSG_ACK: u8 = 5;
pub const DHCP_MSG_NAK: u8 = 6;

// DHCP Option Codes
pub const OPT_PAD: u8 = 0;
pub const OPT_SUBNET_MASK: u8 = 1;
pub const OPT_ROUTER: u8 = 3;
pub const OPT_DNS_SERVER: u8 = 6;
pub const OPT_REQUESTED_IP: u8 = 50;
pub const OPT_LEASE_TIME: u8 = 51;
pub const OPT_MESSAGE_TYPE: u8 = 53;
pub const OPT_SERVER_ID: u8 = 54;
pub const OPT_PARAM_REQUEST_LIST: u8 = 55;
pub const OPT_END: u8 = 255;

pub const DHCP_MIN_HEADER_LEN: usize = 240; // 236 bytes BOOTP header + 4 bytes magic cookie

/// Assigned DHCP Lease parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DhcpLease {
    pub assigned_ip: Ipv4Address,
    pub subnet_mask: Ipv4Address,
    pub gateway: Ipv4Address,
    pub dns_server: Ipv4Address,
    pub lease_time_secs: u32,
    pub server_ip: Ipv4Address,
}

/// Parsed DHCP response information.
#[derive(Debug, Clone, Copy)]
pub struct ParsedDhcpResponse {
    pub op: u8,
    pub xid: u32,
    pub yiaddr: Ipv4Address,
    pub chaddr: MacAddress,
    pub msg_type: Option<u8>,
    pub subnet_mask: Option<Ipv4Address>,
    pub router: Option<Ipv4Address>,
    pub dns_server: Option<Ipv4Address>,
    pub lease_time: Option<u32>,
    pub server_id: Option<Ipv4Address>,
}

impl ParsedDhcpResponse {
    /// Safely parses and bounds-checks a DHCP message and its options.
    pub fn parse(raw: &[u8]) -> Result<Self, &'static str> {
        if raw.len() < DHCP_MIN_HEADER_LEN {
            return Err("Packet shorter than 240-byte BOOTP header with magic cookie");
        }

        let op = raw[0];
        let htype = raw[1];
        let hlen = raw[2];

        if op != DHCP_OP_REPLY && op != DHCP_OP_REQUEST {
            return Err("Invalid BOOTP op code");
        }
        if htype != DHCP_HTYPE_ETHERNET || hlen != DHCP_HLEN_ETHERNET {
            return Err("Unsupported hardware address type in DHCP packet");
        }

        let xid = ((raw[4] as u32) << 24)
            | ((raw[5] as u32) << 16)
            | ((raw[6] as u32) << 8)
            | (raw[7] as u32);

        let mut yiaddr = [0u8; 4];
        yiaddr.copy_from_slice(&raw[16..20]);

        let mut chaddr = [0u8; 6];
        chaddr.copy_from_slice(&raw[28..34]);

        // Validate Magic Cookie (bytes 236..240)
        if raw[236..240] != DHCP_MAGIC_COOKIE {
            return Err("Missing or invalid DHCP magic cookie");
        }

        let mut msg_type = None;
        let mut subnet_mask = None;
        let mut router = None;
        let mut dns_server = None;
        let mut lease_time = None;
        let mut server_id = None;

        // Parse options stream safely with strict bounds checking
        let mut idx = 240;
        while idx < raw.len() {
            let code = raw[idx];
            if code == OPT_PAD {
                idx += 1;
                continue;
            }
            if code == OPT_END {
                break;
            }

            if idx + 1 >= raw.len() {
                return Err("Truncated option header in DHCP options stream");
            }
            let len = raw[idx + 1] as usize;
            idx += 2;

            if idx + len > raw.len() {
                return Err("Option length extends beyond packet boundary");
            }

            let opt_data = &raw[idx..idx + len];
            match code {
                OPT_MESSAGE_TYPE => {
                    if len >= 1 {
                        msg_type = Some(opt_data[0]);
                    }
                }
                OPT_SUBNET_MASK => {
                    if len == 4 {
                        let mut m = [0u8; 4];
                        m.copy_from_slice(opt_data);
                        subnet_mask = Some(m);
                    }
                }
                OPT_ROUTER => {
                    if len >= 4 {
                        let mut r = [0u8; 4];
                        r.copy_from_slice(&opt_data[0..4]);
                        router = Some(r);
                    }
                }
                OPT_DNS_SERVER => {
                    if len >= 4 {
                        let mut d = [0u8; 4];
                        d.copy_from_slice(&opt_data[0..4]);
                        dns_server = Some(d);
                    }
                }
                OPT_LEASE_TIME => {
                    if len == 4 {
                        let secs = ((opt_data[0] as u32) << 24)
                            | ((opt_data[1] as u32) << 16)
                            | ((opt_data[2] as u32) << 8)
                            | (opt_data[3] as u32);
                        lease_time = Some(secs);
                    }
                }
                OPT_SERVER_ID => {
                    if len == 4 {
                        let mut s = [0u8; 4];
                        s.copy_from_slice(opt_data);
                        server_id = Some(s);
                    }
                }
                _ => {}
            }

            idx += len;
        }

        Ok(Self {
            op,
            xid,
            yiaddr,
            chaddr,
            msg_type,
            subnet_mask,
            router,
            dns_server,
            lease_time,
            server_id,
        })
    }
}

/// Constructs a DHCPDISCOVER message into `out_buf`.
pub fn build_dhcp_discover(
    client_mac: MacAddress,
    xid: u32,
    out_buf: &mut [u8],
) -> Result<usize, &'static str> {
    if out_buf.len() < 300 {
        return Err("Output buffer too small for DHCPDISCOVER");
    }

    // Zero entire buffer first
    for b in out_buf.iter_mut() {
        *b = 0;
    }

    out_buf[0] = DHCP_OP_REQUEST; // 1
    out_buf[1] = DHCP_HTYPE_ETHERNET; // 1
    out_buf[2] = DHCP_HLEN_ETHERNET; // 6
    out_buf[3] = 0; // Hops

    // Transaction ID (bytes 4..8)
    out_buf[4] = (xid >> 24) as u8;
    out_buf[5] = (xid >> 16) as u8;
    out_buf[6] = (xid >> 8) as u8;
    out_buf[7] = (xid & 0xFF) as u8;

    // Flags: Broadcast flag 0x8000
    out_buf[8] = 0x80;
    out_buf[9] = 0x00;

    // Client Hardware Address (bytes 28..34)
    out_buf[28..34].copy_from_slice(&client_mac);

    // Magic Cookie (bytes 236..240)
    out_buf[236..240].copy_from_slice(&DHCP_MAGIC_COOKIE);

    let mut idx = 240;

    // Option 53: DHCP Message Type = DHCPDISCOVER (1)
    out_buf[idx] = OPT_MESSAGE_TYPE;
    out_buf[idx + 1] = 1;
    out_buf[idx + 2] = DHCP_MSG_DISCOVER;
    idx += 3;

    // Option 55: Parameter Request List (1 Subnet, 3 Router, 6 DNS)
    out_buf[idx] = OPT_PARAM_REQUEST_LIST;
    out_buf[idx + 1] = 3;
    out_buf[idx + 2] = OPT_SUBNET_MASK;
    out_buf[idx + 3] = OPT_ROUTER;
    out_buf[idx + 4] = OPT_DNS_SERVER;
    idx += 5;

    // Option 255: End
    out_buf[idx] = OPT_END;
    idx += 1;

    // Pad to 300 bytes minimum for standard BOOTP packet
    while idx < 300 {
        out_buf[idx] = 0;
        idx += 1;
    }

    Ok(idx)
}

/// Constructs a DHCPREQUEST message into `out_buf`.
pub fn build_dhcp_request(
    client_mac: MacAddress,
    xid: u32,
    requested_ip: Ipv4Address,
    server_id: Ipv4Address,
    out_buf: &mut [u8],
) -> Result<usize, &'static str> {
    if out_buf.len() < 300 {
        return Err("Output buffer too small for DHCPREQUEST");
    }

    for b in out_buf.iter_mut() {
        *b = 0;
    }

    out_buf[0] = DHCP_OP_REQUEST;
    out_buf[1] = DHCP_HTYPE_ETHERNET;
    out_buf[2] = DHCP_HLEN_ETHERNET;
    out_buf[3] = 0;

    out_buf[4] = (xid >> 24) as u8;
    out_buf[5] = (xid >> 16) as u8;
    out_buf[6] = (xid >> 8) as u8;
    out_buf[7] = (xid & 0xFF) as u8;

    out_buf[8] = 0x80;
    out_buf[9] = 0x00;

    out_buf[28..34].copy_from_slice(&client_mac);
    out_buf[236..240].copy_from_slice(&DHCP_MAGIC_COOKIE);

    let mut idx = 240;

    // Option 53: DHCP Message Type = DHCPREQUEST (3)
    out_buf[idx] = OPT_MESSAGE_TYPE;
    out_buf[idx + 1] = 1;
    out_buf[idx + 2] = DHCP_MSG_REQUEST;
    idx += 3;

    // Option 50: Requested IP Address
    out_buf[idx] = OPT_REQUESTED_IP;
    out_buf[idx + 1] = 4;
    out_buf[idx + 2..idx + 6].copy_from_slice(&requested_ip);
    idx += 6;

    // Option 54: Server Identifier
    out_buf[idx] = OPT_SERVER_ID;
    out_buf[idx + 1] = 4;
    out_buf[idx + 2..idx + 6].copy_from_slice(&server_id);
    idx += 6;

    // Option 55: Parameter Request List
    out_buf[idx] = OPT_PARAM_REQUEST_LIST;
    out_buf[idx + 1] = 3;
    out_buf[idx + 2] = OPT_SUBNET_MASK;
    out_buf[idx + 3] = OPT_ROUTER;
    out_buf[idx + 4] = OPT_DNS_SERVER;
    idx += 5;

    // Option 255: End
    out_buf[idx] = OPT_END;
    idx += 1;

    while idx < 300 {
        out_buf[idx] = 0;
        idx += 1;
    }

    Ok(idx)
}
