#!/usr/bin/env python3
"""RT movie synopsis crawler - calls Rust example."""

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
BINARY = ROOT / "target" / "release" / "examples" / "rt_movies"


def main():
    if not BINARY.exists():
        print(
            "Build the example first: cargo build --release --example rt_movies",
            file=sys.stderr,
        )
        sys.exit(1)

    result = subprocess.run([str(BINARY)], check=False)
    sys.exit(result.returncode)


if __name__ == "__main__":
    main()
