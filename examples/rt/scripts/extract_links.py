#!/usr/bin/env python3
"""RT Stage 1 command transformer: extract movie URLs from browse-page links.

Receives Vec<CrawlResult> JSON on stdin (the raw browser-crawl of the browse page).
Filters result["links"] to RT movie detail pages (www.rottentomatoes.com/m/<slug>),
deduplicates, caps at MAX_MOVIES, and returns a new Vec<CrawlResult> — one entry
per movie URL — so the Pipeline can inject them as Stage 2 inputs via input_from.

Side-effect: writes out/rt/movie_urls.json for inspection.
"""
import json, os, re, sys, time

MAX_MOVIES = 15
# Match https://www.rottentomatoes.com/m/<slug> with no further path segments.
RE_MOVIE = re.compile(r"^https://www\.rottentomatoes\.com/m/([^/?#]+)/?$")

results = json.load(sys.stdin)

seen, movie_urls = set(), []
for r in results:
    for link in r.get("links", []):
        base = link.split("?")[0].rstrip("/")
        if RE_MOVIE.match(base) and base not in seen:
            seen.add(base)
            movie_urls.append(base)
        if len(movie_urls) >= MAX_MOVIES:
            break
    if len(movie_urls) >= MAX_MOVIES:
        break

print(f"rt_links.py: extracted {len(movie_urls)} movie URLs", file=sys.stderr)

os.makedirs("out/rt", exist_ok=True)
with open("out/rt/movie_urls.json", "w") as f:
    json.dump(movie_urls, f, indent=2)

now_secs = int(time.time())
output = [
    {
        "url": url,
        "status_code": None,
        "title": None,
        "content": "",
        "links": [],
        "metadata": {},
        "timestamp": {"secs_since_epoch": now_secs, "nanos_since_epoch": 0},
        "depth": 0,
        "content_type": None,
        "headers": {},
        "errors": [],
    }
    for url in movie_urls
]

print(json.dumps(output))
