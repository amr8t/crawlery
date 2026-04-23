#!/usr/bin/env python3
import json, os, re, sys

RE_EMPTY_IMG  = re.compile(r"""!\[\]\([^)]*\)""")
RE_EMPTY_LINK = re.compile(r"""\[\]\([^)]*\)""")
RE_LINK       = re.compile(r"""\[([^\]]*)\]\(([^)]*)\)""")
RE_RANK       = re.compile(r"""^\d+\.$""")

DISCARD_LABELS = {"hide", "flag", "login", "submit", "more", "welcome",
                  "past", "new", "newest", "front", "comments", "ask",
                  "show", "jobs", "discuss"}


def is_nav_line(line):
    stripped = line.strip()
    if not stripped:
        return False
    labels = RE_LINK.findall(stripped)
    if len(labels) < 3:
        return False
    bare = RE_LINK.sub("", stripped)
    bare = re.sub(r"""[|*_`#>]""", "", bare).strip()
    return len(bare) <= 4


def link_replacer(m):
    label, url = m.group(1).strip(), m.group(2).strip()
    if label.lower() in DISCARD_LABELS:
        return ""
    if url.startswith("http"):
        return m.group(0)
    return label


def clean_meta(line):
    cleaned = RE_LINK.sub(link_replacer, line)
    cleaned = re.sub(r"""\s*\|\s*""", " \u00b7 ", cleaned)
    cleaned = re.sub(r"""(\s*\u00b7\s*)+""", " \u00b7 ", cleaned)
    cleaned = cleaned.strip(" \u00b7").strip()
    cleaned = re.sub(r"""  +""", " ", cleaned)
    return cleaned


def reformat_hn_listing(lines):
    out = []
    n   = len(lines)
    first_rank = next((j for j, l in enumerate(lines) if RE_RANK.match(l.strip())), n)
    header = [l for l in lines[:first_rank] if l.strip()]
    if header:
        first = header[0]
        if not first.startswith(("#", "[", "!")):
            out.append(f"# {first}")
            header = header[1:]
        out.extend(l for l in header if l.strip() and not is_nav_line(l))
        out.append("")

    i = first_rank
    while i < n:
        line = lines[i].strip()
        if RE_RANK.match(line):
            rank = line
            title_line = meta_line = ""
            i += 1
            lookahead = 0
            while i < n and lookahead < 10:
                l = lines[i].strip()
                i += 1
                lookahead += 1
                if not l:
                    continue
                if RE_RANK.match(l):
                    i -= 1
                    break
                if not title_line and l.startswith("[") and "http" in l:
                    title_line = l
                elif not meta_line and "points" in l and "by" in l:
                    meta_line = l
                    break
            if title_line:
                title_clean = RE_LINK.sub(
                    lambda m: m.group(0) if m.group(2).startswith("http") else m.group(1),
                    title_line
                )
                out.append(f"{rank} {title_clean}")
                if meta_line:
                    out.append(f"   {clean_meta(meta_line)}")
                out.append("")
        else:
            if line and not any(kw in line.lower() for kw in (
                    "guidelines", "apply to yc", "consider applying",
                    "search:", "ycombinator.com/legal", "privacy")):
                out.append(line)
            i += 1
    return out


def clean(text):
    text  = RE_EMPTY_IMG.sub("", text)
    text  = RE_EMPTY_LINK.sub("", text)
    lines = [l.rstrip() for l in text.splitlines()]
    lines = [l for l in lines if not is_nav_line(l)]
    if any(RE_RANK.match(l.strip()) for l in lines):
        lines = reformat_hn_listing(lines)
    text = "\n".join(lines)
    text = re.sub(r"\n{3,}", "\n\n", text)
    return text.strip() + "\n"


def save_article_urls(results, out_path, limit=5):
    seen, urls = set(), []
    for r in results:
        for link in r.get("links", []):
            if (link.startswith("http")
                    and "ycombinator.com" not in link
                    and link not in seen):
                seen.add(link)
                urls.append(link)
                if len(urls) >= limit:
                    break
        if len(urls) >= limit:
            break
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    with open(out_path, "w") as f:
        json.dump(urls, f, indent=2)


def main():
    results = json.loads(sys.stdin.read())
    for r in results:
        if isinstance(r.get("content"), str):
            r["content"] = clean(r["content"])
    save_article_urls(results, "out/hn_crawl/article_urls.json")
    print(json.dumps(results))


if __name__ == "__main__":
    main()
