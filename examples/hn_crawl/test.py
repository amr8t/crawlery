#!/usr/bin/env python3
"""HN session crawl test — browser login + HTTP crawl with saved session."""

import string
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))
from bindings import OUT, ensure_binary, load_json, run_recipe

HN_OUT = OUT / "hn_crawl"
RECIPES = Path(__file__).parent
ENV_FILE = Path(__file__).resolve().parent.parent.parent / ".env"


def load_env(path: Path) -> dict:
    """Parse a .env file and return a dict of key/value pairs."""
    env = {}
    if not path.exists():
        print(f"WARNING: .env file not found at {path}", file=sys.stderr)
        return env
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if "=" in line:
            key, _, value = line.partition("=")
            env[key.strip()] = value.strip()
    return env


def render_recipe(recipe_path: Path, env: dict) -> Path:
    """
    Substitute ${VAR} placeholders in a recipe YAML using env values.
    Writes the rendered content to a NamedTemporaryFile and returns its path.
    The caller is responsible for deleting the file when done.
    """
    template = string.Template(recipe_path.read_text())
    rendered = template.safe_substitute(env)

    tmp = tempfile.NamedTemporaryFile(
        mode="w",
        suffix=".yaml",
        prefix=f"{recipe_path.stem}_rendered_",
        delete=False,
    )
    tmp.write(rendered)
    tmp.flush()
    tmp.close()
    return Path(tmp.name)


def run_rendered_recipe(recipe_path: Path, env: dict, timeout: int = 120):
    """Render a recipe template and run it, cleaning up the temp file after."""
    rendered = render_recipe(recipe_path, env)
    try:
        return run_recipe(rendered, timeout=timeout)
    finally:
        rendered.unlink(missing_ok=True)


def main():
    env = load_env(ENV_FILE)

    username = env.get("HN_USERNAME")
    if not username:
        print("ERROR: HN_USERNAME not set in .env", file=sys.stderr)
        sys.exit(1)
    if not env.get("HN_PASSWORD"):
        print("ERROR: HN_PASSWORD not set in .env", file=sys.stderr)
        sys.exit(1)

    ensure_binary()
    print("=== HN Session Crawl Test ===\n")

    # Stage 1: Browser login
    print("--- Login ---")
    run_rendered_recipe(RECIPES / "01_login.yaml", env, timeout=60)
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
    assert username in content, f"Username '{username}' not in scraped content"
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
    run_rendered_recipe(RECIPES / "01_login.yaml", env, timeout=60)
    run_recipe(RECIPES / "02_crawl.yaml")

    results = load_json(HN_OUT / "02_crawl.json")
    assert results and len(results) >= 2, "Pipeline output missing or incomplete"
    content = "\n".join(r.get("content", "") for r in results)
    assert username in content, "Pipeline: username not found"
    print(f"✓ Pipeline OK — {len(results)} pages\n")

    print("=== All tests passed ✓ ===")


if __name__ == "__main__":
    main()
