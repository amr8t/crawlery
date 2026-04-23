#!/usr/bin/env python3
"""
Skeleton for a custom crawlery Command transformer.

Copy this file, implement your logic in `transform_result`, and reference it
in your recipe:

  transformers:
    - type: command
      cmd: python3
      args: ["path/to/your_script.py"]
      timeout_ms: 15000

Each result has these fields (all optional fields may be null/absent):
  url          string
  status_code  int | null
  title        string | null
  content      string          ← the extracted markdown text
  links        [string]
  metadata     {string: string}
  depth        int
  errors       [string]
"""

import json
import sys


def transform_result(result: dict) -> dict:
    """
    Modify a single crawl result. Return the (possibly modified) result.
    To drop a result entirely, return None.
    """
    content = result.get('content', '')

    # ---- put your logic here ----
    # Example: strip lines shorter than 20 chars (removes many nav/UI lines)
    # lines = [l for l in content.splitlines() if len(l.strip()) >= 20 or not l.strip()]
    # result['content'] = '\n'.join(lines)
    # -----------------------------

    return result


def main():
    results = json.loads(sys.stdin.read())
    out = []
    for r in results:
        transformed = transform_result(r)
        if transformed is not None:
            out.append(transformed)
    print(json.dumps(out))


if __name__ == '__main__':
    main()
