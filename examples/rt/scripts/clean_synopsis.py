#!/usr/bin/env python3
"""RT Stage 2 command transformer: clean movie content, extract synopsis.

Receives a Vec<CrawlResult> JSON on stdin after md_readability extraction.
For each result:
  - Finds and isolates the synopsis paragraph from the readability output.
  - Stores the raw synopsis in metadata["synopsis"].
  - Trims content to synopsis + title info (drop review noise).
  - Adds metadata["has_synopsis"] flag used by the Rust fork-join.

Returns the modified Vec<CrawlResult> JSON on stdout.
"""
import json, re, sys

def extract_synopsis(content):
    """Return (synopsis_text, cleaned_content) from readability markdown."""
    lines = content.splitlines()
    synopsis = ""
    synopsis_start = -1

    for i, line in enumerate(lines):
        # readability output often has "Synopsis" as a section marker
        if re.match(r"^\s*synopsis\s*$", line, re.I):
            # grab the next non-empty paragraph
            for j in range(i + 1, min(i + 6, len(lines))):
                candidate = lines[j].strip()
                if len(candidate) > 60:
                    synopsis = candidate
                    synopsis_start = i
                    break
            if synopsis:
                break
        # also catch "SynopsisText..." (no newline between label and text)
        m = re.match(r"^synopsis(.{60,})", line, re.I)
        if m:
            synopsis = m.group(1).strip()
            synopsis_start = i
            break

    # Fallback: first substantial paragraph not starting with "#", reviewer, or "@"
    if not synopsis:
        for line in lines:
            stripped = line.strip()
            if (len(stripped) > 80
                    and not stripped.startswith("#")
                    and not stripped.startswith("@")
                    and not re.match(r"^[A-Z][a-z]+ [A-Z] @", stripped)
                    and "points" not in stripped[:30]):
                synopsis = stripped
                break

    # Build cleaned content: title heading + synopsis only
    title_lines = [l for l in lines if l.startswith("# ")]
    parts = []
    if title_lines:
        parts.append(title_lines[0])
    if synopsis:
        parts.append("")
        parts.append(synopsis)
    cleaned = "\n".join(parts) if parts else content[:600]

    return synopsis, cleaned


results = json.load(sys.stdin)

for r in results:
    content = r.get("content") or ""
    synopsis, cleaned = extract_synopsis(content)
    r["metadata"]["synopsis"] = synopsis
    r["metadata"]["has_synopsis"] = "true" if len(synopsis) > 60 else "false"
    r["content"] = cleaned           # replace with focused content

print(json.dumps(results))
