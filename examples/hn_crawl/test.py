#!/usr/bin/env python3
"""HN session crawl test — browser login + HTTP crawl with saved session."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))
from bindings import OUT, ensure_binary, load_json, run_recipe

USERNAME = "marty9x9"
HN_OUT = OUT / "hn_crawl"
RECIPES = Path(__file__).parent


def main():
    ensure_binary()
    print("=== HN Session Crawl Test ===\n")

    # Stage 1: Browser login
    print("--- Login ---")
    run_recipe(RECIPES / "01_login.yaml")
    session = load_json(HN_OUT / "session.json")
    assert session and session.get("cookies"), "Session file missing or empty"
    cookies = {c["name"]: c["value"] for c in session["cookies"]}
    assert "user" in cookies, f"'user' cookie missing; got: {list(cookies.keys())}"
    print(f"✓ Login OK — saved {len(cookies)} cookie(s)\n")

    # Stage 2: HTTP crawl with session
    print("--- Crawl ---")
    run_recipe(RECIPES / "02_crawl.yaml")
    results = load_json(HN_OUT / "02_crawl.json")
    assert results and len(results) >= 2, f"Expected 2+ pages, got {len(results or [])}"
    assert all(r["status_code"] == 200 for r in results), "Non-200 status codes"
    content = "\n".join(r.get("content", "") for r in results)
    assert USERNAME in content, f"Username '{USERNAME}' not in scraped content"
    print(f"✓ Crawl OK — {len(results)} pages, session verified\n")

    # Stage 3: Full pipeline (programmatic)
    print("--- Pipeline (programmatic) ---")
    for f in [
        HN_OUT / "01_login.json",
        HN_OUT / "02_crawl.json",
        HN_OUT / "session.json",
    ]:
        f.unlink(missing_ok=True)

    # Run stages sequentially
    run_recipe(RECIPES / "01_login.yaml")
    run_recipe(RECIPES / "02_crawl.yaml")

    results = load_json(HN_OUT / "02_crawl.json")
    assert results and len(results) >= 2, "Pipeline output missing or incomplete"
    content = "\n".join(r.get("content", "") for r in results)
    assert USERNAME in content, "Pipeline: username not found"
    print(f"✓ Pipeline OK — {len(results)} pages\n")

    print("=== All tests passed ✓ ===")


if __name__ == "__main__":
    main()
