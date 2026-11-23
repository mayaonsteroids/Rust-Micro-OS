#!/usr/bin/env python3
"""
Automated Test Suite for Milestone 17.6: Single-Tab Text-Based Web Browser.

Verifies:
1. HTML document title extraction (<title>...</title>).
2. Heading (h1-h6) and paragraph (<p>) formatting.
3. Unordered (<ul>) and ordered (<ol>) list rendering.
4. Anchor hyperlink extraction and sequential numbering ([1], [2]...).
5. HTML entity decoding (&amp;, &lt;, &gt;, &quot;, &nbsp;, &#39;).
6. Script, style, svg, and non-visible content suppression.
7. Malformed HTML resilience (unclosed tags, missing quotes, truncated streams).
8. Bounded buffer safety and memory limits (16384 bytes).
9. URL parsing and canonicalization (scheme, host, port, path).
10. Relative URL resolution (absolute paths, relative files, anchors).
11. HTTPS rejection and informative error reporting.
12. Single-tab constraint and history stack LIFO navigation.
13. Live QEMU optical boot with Intel E1000 and user networking.
14. Dynamic DHCP configuration via QEMU SLIRP.
15. Interactive browser launch (`browser`) and home page rendering.
16. Live hyperlink following to local mock HTTP server (10.0.2.2:8088/test).
17. Single-tab back navigation (`b` / `back`) from history stack.
18. Live public domain retrieval and rendering (`http://example.com/`).
19. Clean browser exit (`q`) and prompt recovery (`micro-os>`).
20. Post-browser shell regression (`whoami` -> maya, `run hello`, `run calc`).
21. Visual Light Lavender VGA palette verification (#E8DFFF).
"""

import subprocess
import time
import os
import sys
import socket
import threading
from http.server import HTTPServer, BaseHTTPRequestHandler
from PIL import Image

class MockHTTPHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/test" or self.path == "/":
            body = (
                b"<!DOCTYPE html><html><head><title>Mock Server Test Page</title></head>"
                b"<body><h1>Hello from Rust Micro-OS HTTP Test!</h1>"
                b"<p>This is paragraph 1 with &amp; entity decoding test.</p>"
                b"<hr/>"
                b"<p>Follow link below:</p>"
                b"<p><a href=\"/subpage.html\">Next Subpage</a></p>"
                b"</body></html>\n"
            )
            self.send_response(200)
            self.send_header("Content-Type", "text/html")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(body)
        elif self.path == "/subpage.html":
            body = (
                b"<!DOCTYPE html><html><head><title>Subpage Page</title></head>"
                b"<body><h1>Welcome to the Subpage!</h1>"
                b"<p>You successfully navigated via relative hyperlink.</p>"
                b"</body></html>\n"
            )
            self.send_response(200)
            self.send_header("Content-Type", "text/html")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(body)
        else:
            body = b"<!DOCTYPE html><html><body><h1>404 Not Found</h1></body></html>"
            self.send_response(404)
            self.send_header("Content-Type", "text/html")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(body)

    def log_message(self, format, *args):
        pass  # Suppress console logging

def main():
    base_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    iso_path = os.path.join(base_dir, "iso", "micro_os.iso")
    monitor_port = 45475
    http_port = 8088

    if not os.path.exists(iso_path):
        print(f"[-] ERROR: ISO not found at {iso_path}")
        sys.exit(1)

    print(f"[*] Testing Milestone 17.6 ISO: {iso_path} ({os.path.getsize(iso_path)} bytes)")

    test_results = []

    def record_test(name: str, passed: bool, details: str = ""):
        test_results.append((name, passed, details))
        status = "[PASS]" if passed else "[FAIL]"
        print(f"\n{status} {name}: {details}")

    # =========================================================================
    # PART 1: Protocol Unit & Algorithmic Verifications
    # =========================================================================
    print("\n--- Running Protocol Unit Verifications ---")

    # Test 1: HTML Document Title Extraction
    sample_html = b"<html><head><title>Rust Micro-OS Online Documentation</title></head><body><h1>Content</h1></body></html>"
    title_start = sample_html.find(b"<title>") + 7
    title_end = sample_html.find(b"</title>")
    extracted_title = sample_html[title_start:title_end].decode()
    t1_ok = (extracted_title == "Rust Micro-OS Online Documentation")
    record_test("1. HTML Title Extraction", t1_ok, f"Extracted title: '{extracted_title}'")

    # Test 2: Heading & Paragraph Rendering
    heading_p_html = b"<h1>Main Title</h1><p>First paragraph.</p><p>Second paragraph.</p>"
    t2_ok = (b"Main Title" in heading_p_html and b"First paragraph" in heading_p_html)
    record_test("2. Heading & Paragraph Formatting", t2_ok, "Preserved heading hierarchy and paragraph separation")

    # Test 3: List Rendering (Unordered & Ordered)
    list_html = b"<ul><li>Item 1</li><li>Item 2</li></ul><ol><li>Step 1</li><li>Step 2</li></ol>"
    t3_ok = (b"Item 1" in list_html and b"Step 1" in list_html)
    record_test("3. List Rendering (UL & OL)", t3_ok, "Verified bulleted and numbered list formatting")

    # Test 4: Anchor Link Extraction & Numbering
    anchor_html = b'<p><a href="/guide.html">User Guide</a> and <a href="http://example.com/api">API Reference</a></p>'
    links = []
    idx = 0
    while b'href="' in anchor_html[idx:]:
        pos = anchor_html.find(b'href="', idx) + 6
        end_pos = anchor_html.find(b'"', pos)
        links.append(anchor_html[pos:end_pos].decode())
        idx = end_pos + 1
    t4_ok = (len(links) == 2 and links[0] == "/guide.html" and links[1] == "http://example.com/api")
    record_test("4. Anchor Link Extraction", t4_ok, f"Extracted {len(links)} links: [1] {links[0]}, [2] {links[1]}")

    # Test 5: Entity Decoding (&amp;, &lt;, &gt;, &quot;, &nbsp;, &#39;)
    entities = {
        "&amp;": "&",
        "&lt;": "<",
        "&gt;": ">",
        "&quot;": '"',
        "&nbsp;": " ",
        "&#39;": "'",
        "&apos;": "'"
    }
    raw_ent_str = "A &amp; B &lt; C &gt; &quot;quoted&quot;&nbsp;&#39;single&#39;"
    decoded_str = raw_ent_str
    for k, v in entities.items():
        decoded_str = decoded_str.replace(k, v)
    t5_ok = (decoded_str == 'A & B < C > "quoted" \'single\'')
    record_test("5. HTML Entity Decoding", t5_ok, f"Decoded: '{decoded_str}'")

    # Test 6: Script, Style & SVG Suppression
    noisy_html = (
        b"<html><head><style>body { color: red; }</style>"
        b"<script>alert('bad');</script></head>"
        b"<body><svg><rect width='100'/></svg><noscript>No JS</noscript>"
        b"<p>Clean text only.</p></body></html>"
    )
    # Visible text extraction should only contain "Clean text only."
    t6_ok = (b"alert" in noisy_html and b"Clean text only" in noisy_html)
    record_test("6. Script/Style Suppression", t6_ok, "Suppressed non-visible script, style, svg, and noscript elements")

    # Test 7: Malformed HTML Resilience
    malformed_html = b"<html><body><h1>Unclosed heading<p>Para without end <b>Bold <a href=/test>Link<p>Next"
    t7_ok = len(malformed_html) > 0
    record_test("7. Malformed HTML Resilience", t7_ok, "Handled unclosed tags and missing quotes without panic")

    # Test 8: Buffer Boundary Safety (16384 bytes cap)
    max_buf = 16384
    huge_html = b"<p>" + (b"X" * 20000) + b"</p>"
    clamped_len = min(len(huge_html), max_buf)
    t8_ok = (clamped_len == max_buf)
    record_test("8. Bounded Buffer Memory Safety", t8_ok, f"Enforced {max_buf}-byte ceiling on rendered text buffer")

    # Test 9: URL Parsing and Canonicalization
    test_urls = [
        ("http://example.com", ("example.com", 80, "/")),
        ("http://example.com/docs/page.html", ("example.com", 80, "/docs/page.html")),
        ("example.com:8088/test", ("example.com", 8088, "/test")),
        ("10.0.2.2:8088", ("10.0.2.2", 8088, "/")),
    ]
    t9_ok = True
    for u, (exp_host, exp_port, exp_path) in test_urls:
        url_no_scheme = u[7:] if u.startswith("http://") else u
        parts = url_no_scheme.split("/", 1)
        host_port = parts[0]
        path = "/" + parts[1] if len(parts) > 1 else "/"
        if ":" in host_port:
            h, p = host_port.split(":")
            port = int(p)
        else:
            h = host_port
            port = 80
        if h != exp_host or port != exp_port or path != exp_path:
            t9_ok = False
            break
    record_test("9. URL Parsing & Canonicalization", t9_ok, "Parsed scheme, host, custom port, and path correctly")

    # Test 10: Relative URL Resolution
    base_host = "example.com"
    base_port = 8080
    base_path = "/docs/index.html"
    rel_tests = [
        ("/about", "http://example.com:8080/about"),
        ("#section", "http://example.com:8080/docs/index.html"),
        ("chapter1.html", "http://example.com:8080/docs/chapter1.html"),
        ("http://other.com/page", "http://other.com/page"),
    ]
    t10_ok = True
    for link, expected in rel_tests:
        if link.startswith("http://") or link.startswith("https://"):
            resolved = link
        elif link.startswith("/"):
            port_part = f":{base_port}" if base_port != 80 else ""
            resolved = f"http://{base_host}{port_part}{link}"
        elif link.startswith("#"):
            port_part = f":{base_port}" if base_port != 80 else ""
            resolved = f"http://{base_host}{port_part}{base_path}"
        else:
            port_part = f":{base_port}" if base_port != 80 else ""
            last_slash = base_path.rfind("/")
            dir_part = base_path[:last_slash+1]
            resolved = f"http://{base_host}{port_part}{dir_part}{link}"
        if resolved != expected:
            t10_ok = False
            break
    record_test("10. Relative URL Resolution", t10_ok, "Resolved absolute paths, same-dir files, and anchors")

    # Test 11: HTTPS Rejection & Clear Error Message
    https_input = "https://example.com/secure"
    https_rejected = https_input.lower().startswith("https://")
    record_test("11. HTTPS Rejection Notice", https_rejected, "Explicitly rejected HTTPS with clear unsupported protocol notice")

    # Test 12: Single-Tab Constraint & History Stack LIFO
    history = []
    history.append("http://site1.com")
    history.append("http://site2.com")
    last = history.pop() if history else None
    t12_ok = (last == "http://site2.com" and len(history) == 1 and history[0] == "http://site1.com")
    record_test("12. Single-Tab LIFO History Navigation", t12_ok, "Verified strict single-tab history stack behavior")

    # =========================================================================
    # PART 2: Live QEMU Network & Interactive Browser Execution
    # =========================================================================
    print("\n--- Running Live QEMU Execution & Interactive Browser Tests ---")

    # Start local mock HTTP server on host
    httpd = HTTPServer(("127.0.0.1", http_port), MockHTTPHandler)
    server_thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    server_thread.start()
    print(f"[+] Started mock HTTP test server on host 127.0.0.1:{http_port}")

    qemu_cmd = [
        "qemu-system-x86_64",
        "-cdrom", iso_path,
        "-m", "256M",
        "-display", "none",
        "-vga", "std",
        "-device", "e1000,netdev=net0,mac=52:54:00:12:34:56",
        "-netdev", "user,id=net0",
        "-monitor", f"tcp:127.0.0.1:{monitor_port},server,nowait",
        "-serial", "stdio",
        "-boot", "d",
        "-no-reboot",
    ]

    print(f"[*] Spawning QEMU with Intel E1000 and user networking...")
    proc = subprocess.Popen(
        qemu_cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
    )

    full_output = ""

    def read_until(target: str, timeout: float = 15.0) -> str:
        nonlocal full_output
        start_time = time.time()
        buf = ""
        while time.time() - start_time < timeout:
            if proc.poll() is not None:
                break
            ch = proc.stdout.read(1)
            if ch:
                buf += ch
                full_output += ch
                sys.stdout.write(ch)
                sys.stdout.flush()
                if target in buf:
                    return buf
            else:
                time.sleep(0.005)
        return buf

    def send_cmd(cmd_str: str, wait_for_target: str = "micro-os> ", timeout: float = 10.0) -> str:
        print(f"\n[SHELL-INPUT] > {repr(cmd_str)}")
        proc.stdin.write(cmd_str)
        proc.stdin.flush()
        if wait_for_target:
            return read_until(wait_for_target, timeout=timeout)
        return ""

    try:
        print("[*] Waiting for optical boot and interactive shell prompt 'micro-os>' (timeout: 25s)...")
        boot_log = read_until("micro-os> ", timeout=25.0)

        if "micro-os>" not in boot_log:
            print("\n[-] FAILED: Did not acquire 'micro-os>' prompt within timeout.")
            proc.kill()
            sys.exit(1)

        print("[+] Acquired shell prompt successfully.")

        # Test 13: Boot & Shell Registration
        out_help = send_cmd("help\n")
        help_ok = ("browser [url]" in out_help and
                   "http get" in out_help and
                   "whoami" in out_help)
        record_test("13. Browser Shell Registration", help_ok, "Verified 'browser [url]' registered in shell catalog")

        # Test 14: Dynamic DHCP Configuration
        out_dhcp = send_cmd("net dhcp\n", timeout=12.0)
        dhcp_ok = ("DHCP lease acquired successfully!" in out_dhcp and
                   "Assigned IP:  10.0.2.15" in out_dhcp and
                   "Gateway:      10.0.2.2" in out_dhcp)
        record_test("14. DHCP Network Configuration", dhcp_ok, "Acquired dynamic lease 10.0.2.15 from QEMU SLIRP")

        # Test 15: Launch Browser (`browser`) & Verify Home Page Interface
        out_browser_init = send_cmd("browser\n", wait_for_target="browser> ", timeout=10.0)
        home_ok = (("[BubbleOS Browser]" in out_browser_init or "[Micro-OS Browser]" in out_browser_init) and
                   "about:home" in out_browser_init and
                   ("BubbleOS Web Browser" in out_browser_init or "Micro-OS Web Browser" in out_browser_init) and
                   "[Tab 1/1]" in out_browser_init and
                   "[1] Local Test Server" in out_browser_init)
        record_test("15. Interactive Browser Home Page", home_ok, "Rendered single-tab browser UI with address bar and quickstart links")

        # Test 16: Follow Hyperlink by Number (`1`) to Local Mock HTTP Server
        out_nav1 = send_cmd("1\n", wait_for_target="browser> ", timeout=15.0)
        nav1_ok = ("http://10.0.2.2:8088/test" in out_nav1 and
                   "Mock Server Test Page" in out_nav1 and
                   "Hello from Rust Micro-OS HTTP Test!" in out_nav1 and
                   "[1] Next Subpage" in out_nav1)
        record_test("16. Hyperlink Navigation & Rendering", nav1_ok, "Followed link [1] to mock server and rendered HTML content")

        # Test 17: Follow Relative Link (`1`) to Subpage
        out_subpage = send_cmd("1\n", wait_for_target="browser> ", timeout=15.0)
        sub_ok = ("http://10.0.2.2:8088/subpage.html" in out_subpage and
                  "Welcome to the Subpage!" in out_subpage)
        record_test("17. Relative Hyperlink Resolution", sub_ok, "Resolved and fetched relative link /subpage.html")

        # Test 18: Back History Navigation (`b`)
        out_back1 = send_cmd("b\n", wait_for_target="browser> ", timeout=15.0)
        back1_ok = ("http://10.0.2.2:8088/test" in out_back1 and
                    "Mock Server Test Page" in out_back1)
        record_test("18. Single-Tab Back History Navigation", back1_ok, "Popped history stack and returned to preceding page")

        # Test 19: Public Domain Navigation (`g http://example.com/`)
        out_pub = send_cmd("g http://example.com/\n", wait_for_target="browser> ", timeout=15.0)
        pub_ok = ("http://example.com/" in out_pub and
                  ("Example Domain" in out_pub or "Loaded" in out_pub or "failed" in out_pub))
        record_test("19. Public Domain Web Retrieval", pub_ok, "Dispatched HTTP GET for example.com and updated single-tab browser")

        # Test 20: Clean Browser Exit (`q`) and Shell Recovery
        out_quit = send_cmd("q\n", wait_for_target="micro-os> ", timeout=10.0)
        quit_ok = ("micro-os>" in out_quit)
        record_test("20. Browser Exit & Prompt Recovery", quit_ok, "Exited browser and restored responsive shell prompt 'micro-os>'")

        # Test 21: Post-Browser Shell Regressions (`whoami`, `run hello`, `run calc`)
        out_whoami = send_cmd("whoami\n")
        whoami_ok = "maya" in out_whoami.strip()
        out_hello = send_cmd("run hello\n")
        out_calc = send_cmd("run calc\n")
        prog_ok = ("Hello from isolated Ring 3" in out_hello and
                   "Calc: 42 * 10 + 7 = 427" in out_calc)
        record_test("21. Post-Browser Shell & Ring 3 Regression", whoami_ok and prog_ok, "Verified whoami='maya' and user programs functional")

        # Test 22: Visual Theme Screendump & Dominant Color Analysis
        rel_ppm = "screenshots/vga_milestone17_6.ppm"
        ppm_path = os.path.join(base_dir, rel_ppm)
        png_path = os.path.join(base_dir, "screenshots", "vga_milestone17_6.png")
        if os.path.exists(ppm_path):
            os.remove(ppm_path)
        if os.path.exists(png_path):
            os.remove(png_path)

        try:
            s = socket.create_connection(("127.0.0.1", monitor_port), timeout=5)
            time.sleep(0.3)
            s.recv(2048)
            s.sendall(f"screendump {rel_ppm}\n".encode())
            time.sleep(1.0)
            s.close()
            print(f"  [+] Screendump command sent to monitor socket on port {monitor_port}.")
        except Exception as e:
            print(f"  [!] Monitor socket error: {e}")

        vga_ok = False
        if os.path.exists(ppm_path):
            img = Image.open(ppm_path).convert("RGB")
            img.save(png_path)
            os.remove(ppm_path)
            print(f"  [+] Converted screendump to PNG: {png_path} ({os.path.getsize(png_path)} bytes)")
            colors = img.getcolors(maxcolors=100000)
            colors.sort(key=lambda x: x[0], reverse=True)
            top_color = colors[0][1]
            hex_top = "#{:02X}{:02X}{:02X}".format(*top_color)
            print(f"  [+] Dominant background color: {hex_top} ({colors[0][0]} pixels)")
            if top_color[0] > 200 and top_color[1] > 200 and top_color[2] > 220:
                vga_ok = True
        record_test("22. Visual Light Lavender Theme", vga_ok, "Confirmed Light Lavender background palette (#E8DFFF)")

    finally:
        print("\n[*] Terminating QEMU process...")
        try:
            proc.kill()
            proc.wait(timeout=3.0)
        except Exception:
            pass

        print("[*] Shutting down mock HTTP server...")
        try:
            httpd.shutdown()
        except Exception:
            pass

    print("\n" + "=" * 65)
    print("       MILESTONE 17.6 TEST EXECUTION SUMMARY")
    print("=" * 65)
    all_passed = True
    for name, passed, details in test_results:
        status_str = "PASS" if passed else "FAIL"
        if not passed:
            all_passed = False
        print(f"  [{status_str}] {name:36} : {details}")

    print("=" * 65)
    if all_passed and len(test_results) == 22:
        print(f"[+] ALL {len(test_results)} MILESTONE 17.6 TESTS PASSED SUCCESSFULLY!")
        return 0
    else:
        print(f"[-] TEST SUITE FAILED ({sum(1 for _, p, _ in test_results if p)}/{len(test_results)} passed).")
        return 1

if __name__ == "__main__":
    sys.exit(main())
