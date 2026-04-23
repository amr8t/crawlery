#!/usr/bin/env python3
"""
Hacker News session crawl test — Phase 1 verification.

Tests:
  1. Browser-mode login via JS hook  ->  session.json saved with HN cookies
  2. HTTP-mode crawl of pages 1 and 2 using saved session
  3. Session verification: username appears in scraped page content
  4. Pipeline mode: both stages run end-to-end with --pipeline flag
"""

import subprocess, json, os, sys
from pathlib import Path

ROOT  = Path(__file__).resolve().parent.parent.parent.parent  # project root
BINARY = ROOT / "target" / "release" / "crawlery"
OUT    = ROOT / "out" / "hn_crawl"
RECIPES = ROOT / "examples" / "recipes" / "hn_crawl"
USERNAME = "marty9x9"

PASS, FAIL = 0, 0

def ok(msg):   global PASS; PASS += 1; print(f"  PASS: {msg}")
def fail(msg): global FAIL; FAIL += 1; print(f"  FAIL: {msg}")
def run(*args, timeout=120):
    return subprocess.run([str(BINARY)] + list(args),
                         capture_output=True, text=True, timeout=timeout)

os.makedirs(OUT, exist_ok=True)

print("=== Hacker News Session Crawl — Phase 1 Test ===")
print(f"Binary : {BINARY}")
print(f"Output : {OUT}")
print()

# ------------------------------------------------------------------ #
# Stage 1: Login (browser mode + JS hook)                            #
# ------------------------------------------------------------------ #
print("--- Stage 1: Browser login via JS hook ---")
r = run("--recipe", str(RECIPES / "01_login.yaml"))
if r.returncode != 0:
    fail(f"Login recipe failed:\n{r.stderr[:400]}")
else:
    ok("Login recipe completed without error")

session_file = OUT / "session.json"
if session_file.exists():
    ok("Session file saved")
    session = json.loads(session_file.read_text())
    cookies = {c["name"]: c["value"] for c in session.get("cookies", [])}
    if cookies:
        ok(f"Session has {len(cookies)} cookie(s): {list(cookies.keys())}")
        # HN sets a 'user' cookie on successful login
        if "user" in cookies:
            ok("HN 'user' auth cookie present — login confirmed")
        else:
            fail(f"'user' cookie missing; got: {list(cookies.keys())}")
    else:
        fail("Session file has no cookies — login may have failed")
else:
    fail("Session file not created — browser or JS hook issue")

print()

# ------------------------------------------------------------------ #
# Stage 2: Crawl pages 1 and 2 with saved session (HTTP mode)        #
# ------------------------------------------------------------------ #
print("--- Stage 2: HTTP crawl of HN pages 1 and 2 with session ---")
r = run("--recipe", str(RECIPES / "02_crawl.yaml"))
if r.returncode != 0:
    fail(f"Crawl recipe failed:\n{r.stderr[:400]}")
else:
    ok("Crawl recipe completed")

crawl_file = OUT / "02_crawl.json"
if crawl_file.exists():
    results = json.loads(crawl_file.read_text())
    ok(f"Output has {len(results)} page result(s)")

    if len(results) >= 2:
        ok("Both HN pages 1 and 2 crawled")
    elif len(results) == 1:
        fail("Only 1 page scraped — input_from may not have loaded both URLs")
    else:
        fail("No pages scraped — session may not be working")

    # Verify status codes
    statuses = [r.get("status_code") for r in results]
    if all(s == 200 for s in statuses):
        ok(f"All pages returned HTTP 200 (filter transformer working)")
    else:
        fail(f"Some non-200 statuses: {statuses}")

    # Key session verification: check username appears in content
    found_username = any(
        USERNAME in (r.get("content") or "") or
        USERNAME in (r.get("title") or "")
        for r in results
    )
    if found_username:
        ok(f"Username '{USERNAME}' found in scraped content — session verified")
    else:
        fail(
            f"Username '{USERNAME}' NOT found in scraped content.\n"
            f"  This means the session cookie was not sent or login failed.\n"
            f"  Content snippet: {(results[0].get('content') or '')[:200]}"
        )
else:
    fail("Crawl output file not created")

print()

# ------------------------------------------------------------------ #
# Stage 3: Full pipeline end-to-end                                  #
# ------------------------------------------------------------------ #
print("--- Stage 3: Full pipeline (--pipeline flag) ---")
# Clean outputs so we know pipeline created them
for f in [OUT / "01_login.json", OUT / "02_crawl.json", OUT / "session.json"]:
    f.unlink(missing_ok=True)

r = run("--pipeline", str(RECIPES / "pipeline.yaml"))
if r.returncode != 0:
    fail(f"Pipeline failed:\n{r.stderr[:400]}")
else:
    ok("Pipeline completed without error")
    print(r.stdout[:300])

pipeline_out = OUT / "02_crawl.json"
if pipeline_out.exists():
    data = json.loads(pipeline_out.read_text())
    if len(data) >= 2:
        ok(f"Pipeline produced {len(data)} result(s)")
    else:
        fail(f"Pipeline produced only {len(data)} result(s)")

    found = any(USERNAME in (r.get("content") or "") for r in data)
    if found:
        ok(f"Pipeline: username '{USERNAME}' confirmed in content")
    else:
        fail(f"Pipeline: username not in scraped content")
else:
    fail("Pipeline output file missing")

print()
print(f"=== Results: {PASS} passed, {FAIL} failed ===")
sys.exit(0 if FAIL == 0 else 1)
