//! Small, Bounded HTML-to-Text Parser and Renderer.
//!
//! Extracts document titles, headings, paragraphs, lists, horizontal rules,
//! anchor links, and decodes HTML entities with zero unbounded heap allocations.
//! Ignores scripts, styles, metadata, and non-visible elements safely.

pub const MAX_RENDERED_TEXT_LEN: usize = 16384;
pub const MAX_TITLE_LEN: usize = 128;
pub const MAX_LINKS: usize = 32;
pub const MAX_URL_LEN: usize = 128;
pub const MAX_DISPLAY_LINES: usize = 256;
pub const DISPLAY_LINE_WIDTH: usize = 76;

/// Extracted anchor hyperlink.
#[derive(Clone, Copy)]
pub struct PageLink {
    pub id: usize,
    pub url: [u8; MAX_URL_LEN],
    pub url_len: usize,
}

impl PageLink {
    pub const fn empty() -> Self {
        Self {
            id: 0,
            url: [0u8; MAX_URL_LEN],
            url_len: 0,
        }
    }

    pub fn url_str(&self) -> &str {
        core::str::from_utf8(&self.url[..self.url_len]).unwrap_or("")
    }
}

/// Parsed page content formatted for text-mode display.
pub struct ParsedPage {
    pub title: [u8; MAX_TITLE_LEN],
    pub title_len: usize,
    pub text: [u8; MAX_RENDERED_TEXT_LEN],
    pub text_len: usize,
    pub links: [PageLink; MAX_LINKS],
    pub link_count: usize,
    pub line_offsets: [(usize, usize); MAX_DISPLAY_LINES],
    pub line_count: usize,
    pub is_truncated: bool,
}

impl ParsedPage {
    pub fn new() -> Self {
        Self {
            title: [0u8; MAX_TITLE_LEN],
            title_len: 0,
            text: [0u8; MAX_RENDERED_TEXT_LEN],
            text_len: 0,
            links: [PageLink::empty(); MAX_LINKS],
            link_count: 0,
            line_offsets: [(0, 0); MAX_DISPLAY_LINES],
            line_count: 0,
            is_truncated: false,
        }
    }

    pub fn title_str(&self) -> &str {
        if self.title_len == 0 {
            "Untitled Document"
        } else {
            core::str::from_utf8(&self.title[..self.title_len]).unwrap_or("Untitled Document")
        }
    }

    pub fn text_str(&self) -> &str {
        core::str::from_utf8(&self.text[..self.text_len]).unwrap_or("")
    }

    pub fn get_line(&self, line_idx: usize) -> &str {
        if line_idx >= self.line_count {
            ""
        } else {
            let (start, end) = self.line_offsets[line_idx];
            if start <= end && end <= self.text_len {
                core::str::from_utf8(&self.text[start..end]).unwrap_or("")
            } else {
                ""
            }
        }
    }
}

/// Helper to append a single ASCII byte into the rendered text buffer.
fn append_byte(page: &mut ParsedPage, b: u8) {
    if page.text_len < MAX_RENDERED_TEXT_LEN {
        page.text[page.text_len] = b;
        page.text_len += 1;
    } else {
        page.is_truncated = true;
    }
}

/// Helper to append a string slice into the rendered text buffer.
fn append_str(page: &mut ParsedPage, s: &str) {
    for b in s.bytes() {
        append_byte(page, b);
    }
}

/// Case-insensitive ASCII prefix match.
fn starts_with_ignore_case(slice: &[u8], prefix: &[u8]) -> bool {
    if slice.len() < prefix.len() {
        return false;
    }
    for i in 0..prefix.len() {
        if slice[i].to_ascii_lowercase() != prefix[i].to_ascii_lowercase() {
            return false;
        }
    }
    true
}

/// Parses an HTML document into a structured `ParsedPage`.
pub fn parse_html(raw_html: &[u8]) -> ParsedPage {
    let mut page = ParsedPage::new();
    let len = raw_html.len();
    let mut i = 0;

    let mut in_script = false;
    let mut in_style = false;
    let mut in_head = false;
    let mut in_svg = false;
    let mut in_noscript = false;

    let mut list_ordered = false;
    let mut list_index = 0usize;
    let mut consecutive_newlines = 2usize;
    let mut last_was_space = true;

    while i < len {
        // 1. Check for HTML comments: <!-- ... -->
        if raw_html[i..].starts_with(b"<!--") {
            i += 4;
            while i + 2 < len && !(raw_html[i] == b'-' && raw_html[i + 1] == b'-' && raw_html[i + 2] == b'>') {
                i += 1;
            }
            if i + 2 < len {
                i += 3;
            } else {
                i = len;
            }
            continue;
        }

        // 2. Check for HTML tag start '<'
        if raw_html[i] == b'<' {
            let tag_start = i + 1;
            // Find tag end '>'
            let mut tag_end = tag_start;
            let mut in_quote = false;
            let mut quote_char = 0u8;

            while tag_end < len {
                let b = raw_html[tag_end];
                if in_quote {
                    if b == quote_char {
                        in_quote = false;
                    }
                } else if b == b'"' || b == b'\'' {
                    in_quote = true;
                    quote_char = b;
                } else if b == b'>' {
                    break;
                }
                tag_end += 1;
            }

            let tag_content = if tag_end <= len {
                &raw_html[tag_start..tag_end]
            } else {
                &raw_html[tag_start..len]
            };

            // Advance past '>'
            i = if tag_end < len { tag_end + 1 } else { len };

            // Extract tag name (handling both opening <tag> and closing </tag>)
            let mut start = 0;
            let is_closing = if tag_content.starts_with(b"/") {
                start = 1;
                true
            } else {
                false
            };
            let mut name_end = start;
            while name_end < tag_content.len() && !tag_content[name_end].is_ascii_whitespace() && tag_content[name_end] != b'/' {
                name_end += 1;
            }
            let clean_name = &tag_content[start..name_end];

            // Match tags (case-insensitive)
            if starts_with_ignore_case(clean_name, b"title") {
                if !is_closing {
                    // Extract text until </title>
                    let title_start = i;
                    while i < len && !starts_with_ignore_case(&raw_html[i..], b"</title>") {
                        i += 1;
                    }
                    let title_bytes = &raw_html[title_start..i];
                    let copy_len = core::cmp::min(title_bytes.len(), MAX_TITLE_LEN);
                    // Filter whitespace in title
                    let mut t_idx = 0;
                    let mut prev_space = true;
                    for &tb in &title_bytes[..copy_len] {
                        if tb == b'\r' || tb == b'\n' || tb == b'\t' || tb == b' ' {
                            if !prev_space && t_idx < MAX_TITLE_LEN {
                                page.title[t_idx] = b' ';
                                t_idx += 1;
                                prev_space = true;
                            }
                        } else if tb >= 0x20 && tb <= 0x7E && t_idx < MAX_TITLE_LEN {
                            page.title[t_idx] = tb;
                            t_idx += 1;
                            prev_space = false;
                        }
                    }
                    page.title_len = t_idx;

                    // Skip </title>
                    if i + 8 <= len && starts_with_ignore_case(&raw_html[i..], b"</title>") {
                        i += 8;
                    }
                }
                continue;
            } else if starts_with_ignore_case(clean_name, b"script") {
                in_script = !is_closing;
                continue;
            } else if starts_with_ignore_case(clean_name, b"style") {
                in_style = !is_closing;
                continue;
            } else if starts_with_ignore_case(clean_name, b"head") {
                in_head = !is_closing;
                continue;
            } else if starts_with_ignore_case(clean_name, b"body") {
                in_head = false;
                continue;
            } else if starts_with_ignore_case(clean_name, b"svg") {
                in_svg = !is_closing;
                continue;
            } else if starts_with_ignore_case(clean_name, b"noscript") {
                in_noscript = !is_closing;
                continue;
            }

            // If we are currently inside non-visible content, skip tag effects
            if in_script || in_style || in_head || in_svg || in_noscript {
                continue;
            }

            // Headings h1..h6
            if clean_name.len() == 2 && clean_name[0].to_ascii_lowercase() == b'h' && (b'1'..=b'6').contains(&clean_name[1]) {
                if is_closing {
                    while consecutive_newlines < 2 {
                        append_byte(&mut page, b'\n');
                        consecutive_newlines += 1;
                    }
                } else {
                    while consecutive_newlines < 2 {
                        append_byte(&mut page, b'\n');
                        consecutive_newlines += 1;
                    }
                    let level = (clean_name[1] - b'0') as usize;
                    for _ in 0..core::cmp::min(level, 3) {
                        append_byte(&mut page, b'#');
                    }
                    append_byte(&mut page, b' ');
                    consecutive_newlines = 0;
                    last_was_space = true;
                }
                continue;
            }

            // Paragraphs
            if starts_with_ignore_case(clean_name, b"p") {
                while consecutive_newlines < 2 {
                    append_byte(&mut page, b'\n');
                    consecutive_newlines += 1;
                }
                last_was_space = true;
                continue;
            }

            // Line breaks
            if starts_with_ignore_case(clean_name, b"br") {
                append_byte(&mut page, b'\n');
                consecutive_newlines = 1;
                last_was_space = true;
                continue;
            }

            // Horizontal rules
            if starts_with_ignore_case(clean_name, b"hr") {
                if consecutive_newlines == 0 {
                    append_byte(&mut page, b'\n');
                }
                append_str(&mut page, "------------------------------------------------------------\n");
                consecutive_newlines = 1;
                last_was_space = true;
                continue;
            }

            // Lists
            if starts_with_ignore_case(clean_name, b"ul") {
                if !is_closing {
                    list_ordered = false;
                }
                while consecutive_newlines < 1 {
                    append_byte(&mut page, b'\n');
                    consecutive_newlines += 1;
                }
                continue;
            }
            if starts_with_ignore_case(clean_name, b"ol") {
                if !is_closing {
                    list_ordered = true;
                    list_index = 0;
                }
                while consecutive_newlines < 1 {
                    append_byte(&mut page, b'\n');
                    consecutive_newlines += 1;
                }
                continue;
            }
            if starts_with_ignore_case(clean_name, b"li") {
                if !is_closing {
                    if consecutive_newlines == 0 {
                        append_byte(&mut page, b'\n');
                    }
                    if list_ordered {
                        list_index += 1;
                        append_str(&mut page, "  ");
                        if list_index < 10 {
                            append_byte(&mut page, b'0' + (list_index as u8));
                        } else {
                            append_byte(&mut page, b'0' + ((list_index / 10) as u8));
                            append_byte(&mut page, b'0' + ((list_index % 10) as u8));
                        }
                        append_str(&mut page, ". ");
                    } else {
                        append_str(&mut page, "  * ");
                    }
                    consecutive_newlines = 0;
                    last_was_space = true;
                }
                continue;
            }

            // Anchor links: <a href="...">
            if starts_with_ignore_case(clean_name, b"a") {
                if !is_closing {
                    // Extract href attribute
                    if let Some(href) = extract_attribute(tag_content, b"href") {
                        if !href.is_empty() && page.link_count < MAX_LINKS {
                            let link_id = page.link_count + 1;
                            let mut link = PageLink::empty();
                            link.id = link_id;
                            let copy_len = core::cmp::min(href.len(), MAX_URL_LEN);
                            link.url[..copy_len].copy_from_slice(&href[..copy_len]);
                            link.url_len = copy_len;
                            page.links[page.link_count] = link;
                            page.link_count += 1;

                            // Emit inline link indicator: [N]
                            if !last_was_space && consecutive_newlines == 0 {
                                append_byte(&mut page, b' ');
                            }
                            append_byte(&mut page, b'[');
                            if link_id < 10 {
                                append_byte(&mut page, b'0' + (link_id as u8));
                            } else {
                                append_byte(&mut page, b'0' + ((link_id / 10) as u8));
                                append_byte(&mut page, b'0' + ((link_id % 10) as u8));
                            }
                            append_str(&mut page, "] ");
                            consecutive_newlines = 0;
                            last_was_space = true;
                        }
                    }
                }
                continue;
            }

            // All other tags stripped without rendering
            continue;
        }

        // If inside suppressed blocks, skip plain text
        if in_script || in_style || in_head || in_svg || in_noscript {
            i += 1;
            continue;
        }

        // 3. Check for HTML entities: &...;
        if raw_html[i] == b'&' {
            let entity_start = i + 1;
            let mut entity_end = entity_start;
            while entity_end < len && (entity_end - entity_start) < 10 && raw_html[entity_end] != b';' && raw_html[entity_end] != b'<' && !raw_html[entity_end].is_ascii_whitespace() {
                entity_end += 1;
            }

            if entity_end < len && raw_html[entity_end] == b';' {
                let ent_name = &raw_html[entity_start..entity_end];
                i = entity_end + 1;

                let decoded_char = match ent_name {
                    b"amp" => Some(b'&'),
                    b"lt" => Some(b'<'),
                    b"gt" => Some(b'>'),
                    b"quot" => Some(b'"'),
                    b"apos" | b"#39" => Some(b'\''),
                    b"nbsp" => Some(b' '),
                    b"#34" => Some(b'"'),
                    b"#38" => Some(b'&'),
                    b"#60" => Some(b'<'),
                    b"#62" => Some(b'>'),
                    _ => None,
                };

                if let Some(ch) = decoded_char {
                    if ch == b' ' {
                        if !last_was_space {
                            append_byte(&mut page, b' ');
                            last_was_space = true;
                        }
                    } else {
                        append_byte(&mut page, ch);
                        consecutive_newlines = 0;
                        last_was_space = false;
                    }
                } else {
                    // Unknown entity: emit bare content without crash
                    append_byte(&mut page, b'&');
                    for &b in ent_name {
                        append_byte(&mut page, b);
                    }
                    append_byte(&mut page, b';');
                    consecutive_newlines = 0;
                    last_was_space = false;
                }
                continue;
            }
        }

        // 4. Regular visible text byte
        let b = raw_html[i];
        i += 1;

        if b == b'\r' || b == b'\n' || b == b'\t' || b == b' ' {
            if !last_was_space && consecutive_newlines == 0 {
                append_byte(&mut page, b' ');
                last_was_space = true;
            }
        } else if (0x20..=0x7E).contains(&b) {
            append_byte(&mut page, b);
            consecutive_newlines = 0;
            last_was_space = false;
        }
    }

    // Wrap text into display lines
    calculate_line_offsets(&mut page);

    page
}

/// Extracts an attribute value from a raw tag slice (e.g. href="...").
fn extract_attribute<'a>(tag: &'a [u8], attr_name: &[u8]) -> Option<&'a [u8]> {
    let mut idx = 0;
    while idx + attr_name.len() < tag.len() {
        if starts_with_ignore_case(&tag[idx..], attr_name) {
            let mut eq_pos = idx + attr_name.len();
            while eq_pos < tag.len() && tag[eq_pos].is_ascii_whitespace() {
                eq_pos += 1;
            }
            if eq_pos < tag.len() && tag[eq_pos] == b'=' {
                let mut val_start = eq_pos + 1;
                while val_start < tag.len() && tag[val_start].is_ascii_whitespace() {
                    val_start += 1;
                }
                if val_start < tag.len() {
                    let first = tag[val_start];
                    if first == b'"' || first == b'\'' {
                        val_start += 1;
                        let mut val_end = val_start;
                        while val_end < tag.len() && tag[val_end] != first {
                            val_end += 1;
                        }
                        return Some(&tag[val_start..val_end]);
                    } else {
                        let mut val_end = val_start;
                        while val_end < tag.len() && !tag[val_end].is_ascii_whitespace() && tag[val_end] != b'>' {
                            val_end += 1;
                        }
                        return Some(&tag[val_start..val_end]);
                    }
                }
            }
        }
        idx += 1;
    }
    None
}

/// Word-wraps `page.text` into visual display lines of max width `DISPLAY_LINE_WIDTH`.
fn calculate_line_offsets(page: &mut ParsedPage) {
    let text = &page.text[..page.text_len];
    let len = text.len();
    if len == 0 {
        page.line_count = 0;
        return;
    }

    let mut line_count = 0;
    let mut line_start = 0;

    while line_start < len && line_count < MAX_DISPLAY_LINES {
        // If current character is newline, record empty or single break line
        if text[line_start] == b'\n' {
            page.line_offsets[line_count] = (line_start, line_start);
            line_count += 1;
            line_start += 1;
            continue;
        }

        // Search for explicit newline or wrap point up to DISPLAY_LINE_WIDTH
        let max_line_end = core::cmp::min(line_start + DISPLAY_LINE_WIDTH, len);
        let mut explicit_nl = None;

        for pos in line_start..max_line_end {
            if text[pos] == b'\n' {
                explicit_nl = Some(pos);
                break;
            }
        }

        if let Some(nl_pos) = explicit_nl {
            page.line_offsets[line_count] = (line_start, nl_pos);
            line_count += 1;
            line_start = nl_pos + 1;
        } else if max_line_end == len {
            // Reached end of text
            page.line_offsets[line_count] = (line_start, len);
            line_count += 1;
            break;
        } else {
            // Need to wrap: search backwards for space
            let mut wrap_at = max_line_end;
            let mut found_space = false;

            for pos in (line_start + (DISPLAY_LINE_WIDTH / 2)..max_line_end).rev() {
                if text[pos] == b' ' {
                    wrap_at = pos;
                    found_space = true;
                    break;
                }
            }

            page.line_offsets[line_count] = (line_start, wrap_at);
            line_count += 1;

            if found_space {
                line_start = wrap_at + 1; // Skip space at wrap
            } else {
                line_start = max_line_end;
            }
        }
    }

    page.line_count = line_count;
}

/// Parsed URL structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedUrl<'a> {
    pub is_https: bool,
    pub host: &'a str,
    pub port: u16,
    pub path: &'a str,
}

/// Parses an input URL string into host, port, path, and checks for HTTPS.
pub fn parse_url(input: &str) -> Result<ParsedUrl<'_>, &'static str> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Empty URL provided");
    }

    // Check for HTTPS
    if starts_with_ignore_case(trimmed.as_bytes(), b"https://") {
        return Err("HTTPS is not supported yet (Milestone 17.5/17.6 HTTP only). Please use an http:// URL.");
    }

    let url_without_scheme = if starts_with_ignore_case(trimmed.as_bytes(), b"http://") {
        &trimmed[7..]
    } else {
        trimmed
    };

    if url_without_scheme.is_empty() {
        return Err("Invalid URL: missing host");
    }

    // Find first slash separating host:port from path
    let (host_port, path) = if let Some(slash_idx) = url_without_scheme.find('/') {
        (&url_without_scheme[..slash_idx], &url_without_scheme[slash_idx..])
    } else {
        (url_without_scheme, "/")
    };

    if host_port.is_empty() {
        return Err("Invalid URL: empty hostname");
    }

    // Parse port if specified in host:port
    let (host, port) = if let Some(colon_idx) = host_port.rfind(':') {
        let h = &host_port[..colon_idx];
        let p_str = &host_port[colon_idx + 1..];
        let mut p_val = 0u32;
        if p_str.is_empty() {
            return Err("Invalid port in URL");
        }
        for b in p_str.bytes() {
            if !b.is_ascii_digit() {
                return Err("Non-numeric port in URL");
            }
            p_val = p_val * 10 + (b - b'0') as u32;
            if p_val > 65535 {
                return Err("Port number out of range (1-65535)");
            }
        }
        (h, p_val as u16)
    } else {
        (host_port, 80)
    };

    let clean_path = if path.is_empty() { "/" } else { path };

    Ok(ParsedUrl {
        is_https: false,
        host,
        port,
        path: clean_path,
    })
}

/// Resolves a potentially relative link against the active base URL.
pub fn resolve_relative_url(
    base: &ParsedUrl,
    link: &str,
    out_buf: &mut [u8],
) -> Result<usize, &'static str> {
    let link = link.trim();
    if link.is_empty() {
        return Err("Empty link target");
    }

    let mut idx = 0;
    let write_bytes = |idx: &mut usize, bytes: &[u8], buf: &mut [u8]| -> Result<(), &'static str> {
        if *idx + bytes.len() > buf.len() {
            return Err("Resolved URL buffer overflow");
        }
        buf[*idx..*idx + bytes.len()].copy_from_slice(bytes);
        *idx += bytes.len();
        Ok(())
    };

    // Full URL
    if starts_with_ignore_case(link.as_bytes(), b"http://") || starts_with_ignore_case(link.as_bytes(), b"https://") {
        write_bytes(&mut idx, link.as_bytes(), out_buf)?;
        return Ok(idx);
    }

    // Same host, absolute path (e.g. "/about")
    if link.starts_with('/') {
        write_bytes(&mut idx, b"http://", out_buf)?;
        write_bytes(&mut idx, base.host.as_bytes(), out_buf)?;
        if base.port != 80 {
            write_bytes(&mut idx, b":", out_buf)?;
            let mut port_str = [0u8; 6];
            let mut p = base.port;
            let mut p_len = 0;
            while p > 0 {
                port_str[p_len] = b'0' + (p % 10) as u8;
                p /= 10;
                p_len += 1;
            }
            for i in (0..p_len).rev() {
                write_bytes(&mut idx, &[port_str[i]], out_buf)?;
            }
        }
        write_bytes(&mut idx, link.as_bytes(), out_buf)?;
        return Ok(idx);
    }

    // In-page anchor (e.g. "#section")
    if link.starts_with('#') {
        write_bytes(&mut idx, b"http://", out_buf)?;
        write_bytes(&mut idx, base.host.as_bytes(), out_buf)?;
        if base.port != 80 {
            write_bytes(&mut idx, b":", out_buf)?;
            // write port
            let mut port_str = [0u8; 6];
            let mut p = base.port;
            let mut p_len = 0;
            while p > 0 {
                port_str[p_len] = b'0' + (p % 10) as u8;
                p /= 10;
                p_len += 1;
            }
            for i in (0..p_len).rev() {
                write_bytes(&mut idx, &[port_str[i]], out_buf)?;
            }
        }
        write_bytes(&mut idx, base.path.as_bytes(), out_buf)?;
        return Ok(idx);
    }

    // Relative path (e.g. "page.html")
    write_bytes(&mut idx, b"http://", out_buf)?;
    write_bytes(&mut idx, base.host.as_bytes(), out_buf)?;
    if base.port != 80 {
        write_bytes(&mut idx, b":", out_buf)?;
        let mut port_str = [0u8; 6];
        let mut p = base.port;
        let mut p_len = 0;
        while p > 0 {
            port_str[p_len] = b'0' + (p % 10) as u8;
            p /= 10;
            p_len += 1;
        }
        for i in (0..p_len).rev() {
            write_bytes(&mut idx, &[port_str[i]], out_buf)?;
        }
    }

    // Append directory from base path
    let last_slash = base.path.rfind('/').unwrap_or(0);
    let base_dir = &base.path[..=last_slash];
    write_bytes(&mut idx, base_dir.as_bytes(), out_buf)?;
    write_bytes(&mut idx, link.as_bytes(), out_buf)?;

    Ok(idx)
}
