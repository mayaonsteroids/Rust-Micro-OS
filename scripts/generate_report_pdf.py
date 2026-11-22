import re
import os
import subprocess
import sys

def md_to_html(md_text):
    # CSS Styles for professional academic report
    css = """
    <style>
        @page {
            size: A4;
            margin: 20mm 15mm 20mm 15mm;
            @bottom-right {
                content: counter(page);
            }
        }
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            color: #24292e;
            line-height: 1.6;
            font-size: 11pt;
            margin: 0;
            padding: 0;
        }
        h1 {
            color: #0366d6;
            font-size: 22pt;
            border-bottom: 2px solid #eaecef;
            padding-bottom: 0.3em;
            margin-top: 0;
            margin-bottom: 8px;
        }
        h2 {
            color: #1a202c;
            font-size: 15pt;
            border-bottom: 1px solid #eaecef;
            padding-bottom: 0.3em;
            margin-top: 24px;
            margin-bottom: 12px;
            page-break-after: avoid;
        }
        h3 {
            color: #2d3748;
            font-size: 12pt;
            margin-top: 16px;
            margin-bottom: 8px;
            page-break-after: avoid;
        }
        p {
            margin-top: 0;
            margin-bottom: 10px;
        }
        table {
            border-collapse: collapse;
            width: 100%;
            margin-top: 12px;
            margin-bottom: 16px;
            font-size: 9.5pt;
            page-break-inside: avoid;
        }
        th, td {
            border: 1px solid #cbd5e0;
            padding: 6px 10px;
            text-align: left;
        }
        th {
            background-color: #f7fafc;
            color: #2d3748;
            font-weight: 600;
        }
        tr:nth-child(even) {
            background-color: #f8fafc;
        }
        pre {
            background-color: #f6f8fa;
            border: 1px solid #e1e4e8;
            border-radius: 4px;
            padding: 10px 12px;
            font-size: 8.5pt;
            font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, Courier, monospace;
            overflow-x: auto;
            margin-top: 8px;
            margin-bottom: 12px;
            page-break-inside: avoid;
        }
        code {
            font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, Courier, monospace;
            font-size: 85%;
            background-color: rgba(27,31,35,0.05);
            padding: 0.2em 0.4em;
            border-radius: 3px;
        }
        pre code {
            background-color: transparent;
            padding: 0;
            font-size: 8.5pt;
        }
        blockquote {
            border-left: 4px solid #0366d6;
            margin: 0;
            padding: 0 15px;
            color: #586069;
        }
        ul, ol {
            margin-top: 0;
            margin-bottom: 12px;
            padding-left: 24px;
        }
        li {
            margin-bottom: 4px;
        }
        hr {
            border: 0;
            border-top: 1px solid #e1e4e8;
            margin: 20px 0;
        }
        .header-box {
            background-color: #f1f8ff;
            border: 1px solid #c8e1ff;
            border-radius: 6px;
            padding: 12px 16px;
            margin-bottom: 20px;
        }
        .header-box p {
            margin: 4px 0;
            font-size: 10pt;
        }
    </style>
    """

    lines = md_text.splitlines()
    html_lines = []
    in_code = False
    in_table = False
    in_list = False

    i = 0
    while i < len(lines):
        line = lines[i]

        # Code blocks
        if line.startswith("```"):
            if in_code:
                html_lines.append("</code></pre>")
                in_code = False
            else:
                if in_list:
                    html_lines.append("</ul>")
                    in_list = False
                if in_table:
                    html_lines.append("</tbody></table>")
                    in_table = False
                html_lines.append("<pre><code>")
                in_code = True
            i += 1
            continue

        if in_code:
            escaped = (line.replace("&", "&amp;")
                           .replace("<", "&lt;")
                           .replace(">", "&gt;"))
            html_lines.append(escaped)
            i += 1
            continue

        # Tables
        if "|" in line and "-|-" in line:
            # Separator line, ignore
            i += 1
            continue
        elif line.startswith("|") and line.endswith("|"):
            cells = [c.strip() for c in line.strip()[1:-1].split("|")]
            if not in_table:
                if in_list:
                    html_lines.append("</ul>")
                    in_list = False
                html_lines.append("<table><thead><tr>")
                for c in cells:
                    html_lines.append(f"<th>{inline_format(c)}</th>")
                html_lines.append("</tr></thead><tbody>")
                in_table = True
            else:
                html_lines.append("<tr>")
                for c in cells:
                    html_lines.append(f"<td>{inline_format(c)}</td>")
                html_lines.append("</tr>")
            i += 1
            continue
        elif in_table:
            html_lines.append("</tbody></table>")
            in_table = False

        # Lists
        if line.startswith("- ") or line.startswith("* "):
            if not in_list:
                html_lines.append("<ul>")
                in_list = True
            content = line[2:].strip()
            html_lines.append(f"<li>{inline_format(content)}</li>")
            i += 1
            continue
        elif in_list and line.strip() == "":
            html_lines.append("</ul>")
            in_list = False

        # Headers
        if line.startswith("# "):
            html_lines.append(f"<h1>{inline_format(line[2:])}</h1>")
        elif line.startswith("## "):
            html_lines.append(f"<h2>{inline_format(line[3:])}</h2>")
        elif line.startswith("### "):
            html_lines.append(f"<h3>{inline_format(line[4:])}</h3>")
        elif line.startswith("---"):
            html_lines.append("<hr>")
        elif line.strip() != "":
            html_lines.append(f"<p>{inline_format(line)}</p>")

        i += 1

    if in_table:
        html_lines.append("</tbody></table>")
    if in_list:
        html_lines.append("</ul>")
    if in_code:
        html_lines.append("</code></pre>")

    return f"<!DOCTYPE html><html><head><meta charset='utf-8'><title>Rust Micro-OS Final Report</title>{css}</head><body>" + "\n".join(html_lines) + "</body></html>"

def inline_format(text):
    # Escape HTML
    t = text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
    # Bold **text**
    t = re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', t)
    # Inline code `code`
    t = re.sub(r'`(.+?)`', r'<code>\1</code>', t)
    # Math $math$
    t = re.sub(r'\$(.+?)\$', r'<em>\1</em>', t)
    # Links [title](url)
    t = re.sub(r'\[(.+?)\]\((.+?)\)', r'<a href="\2">\1</a>', t)
    return t

def main():
    base_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    md_path = os.path.join(base_dir, "docs/FINAL_PROJECT_REPORT.md")
    html_path = os.path.join(base_dir, "docs/FINAL_PROJECT_REPORT.html")
    pdf_path = os.path.join(base_dir, "docs/FINAL_PROJECT_REPORT.pdf")

    with open(md_path, "r", encoding="utf-8") as f:
        md_text = f.read()

    html_content = md_to_html(md_text)
    with open(html_path, "w", encoding="utf-8") as f:
        f.write(html_content)

    print(f"[+] Generated HTML: {html_path} ({os.path.getsize(html_path)} bytes)")

    # Convert to PDF using Microsoft Edge headless
    edge_paths = [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ]
    edge_exe = None
    for p in edge_paths:
        if os.path.exists(p):
            edge_exe = p
            break

    if not edge_exe:
        print("[-] Error: Microsoft Edge executable not found.")
        sys.exit(1)

    cmd = [
        edge_exe,
        "--headless",
        "--disable-gpu",
        "--run-all-compositor-stages-before-draw",
        f"--print-to-pdf={pdf_path}",
        html_path
    ]
    print(f"[*] Converting to PDF using Microsoft Edge headless...")
    res = subprocess.run(cmd, capture_output=True, text=True)
    if os.path.exists(pdf_path) and os.path.getsize(pdf_path) > 1000:
        pdf_size = os.path.getsize(pdf_path)
        print(f"[+] Successfully generated PDF report: {pdf_path} ({pdf_size} bytes / {pdf_size/1024:.1f} KiB)")
    else:
        print(f"[-] Failed to generate PDF: {res.stderr}")
        sys.exit(1)

if __name__ == "__main__":
    main()
