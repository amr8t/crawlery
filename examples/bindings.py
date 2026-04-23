#!/usr/bin/env python3
"""Common Python bindings for invoking crawlery CLI from examples."""

import json
import subprocess
import sys
from pathlib import Path

# Project paths
ROOT = Path(__file__).resolve().parent.parent
BINARY = ROOT / "target" / "release" / "crawlery"
OUT = ROOT / "out"


def run(*args, timeout=120, check=True):
    """
    Run crawlery with given arguments.

    Args:
        *args: CLI arguments to pass to crawlery
        timeout: Command timeout in seconds
        check: Raise CalledProcessError on non-zero exit

    Returns:
        CompletedProcess with stdout, stderr, returncode
    """
    cmd = [str(BINARY)] + [str(arg) for arg in args]
    return subprocess.run(
        cmd, capture_output=True, text=True, timeout=timeout, check=check
    )


def run_recipe(recipe_path, timeout=120):
    """Run a single recipe file."""
    return run("--recipe", recipe_path, timeout=timeout)


def load_json(path):
    """Load and parse JSON file, return None if missing."""
    p = Path(path)
    return json.loads(p.read_text()) if p.exists() else None


def ensure_binary():
    """Check that crawlery binary exists, print helpful error if not."""
    if not BINARY.exists():
        print(f"ERROR: Binary not found at {BINARY}", file=sys.stderr)
        print("Run: cargo build --release", file=sys.stderr)
        sys.exit(1)
