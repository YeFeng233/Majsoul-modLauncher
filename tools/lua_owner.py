#!/usr/bin/env python3
"""Find which function a piece of Lua code lives in.

The dump is minified: one huge line per module, so "which function is this?" cannot be
answered by looking at lines. This walks backwards from each match to the nearest
`function X:y(` / `function X.y(` / `local function z(` and reports it.

    python tools/lua_owner.py PATTERN [--only SUBSTR] [--window 160] [--max 12]
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

DEFAULT_DUMP = Path(r"E:\MajSoul\GameClient\MajsoulPreview\dump")
FUNCTION = re.compile(r"(?:local\s+)?function\s+([A-Za-z_][\w]*(?:[:.][A-Za-z_][\w]*)?)\s*\(")


def owner_of(text: str, position: int) -> str:
    best = "(top level)"
    for match in FUNCTION.finditer(text, 0, position):
        best = match.group(1)
    return best


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("pattern")
    parser.add_argument("--only", default="")
    parser.add_argument("--window", type=int, default=150)
    parser.add_argument("--max", type=int, default=12)
    parser.add_argument("--dump", type=Path, default=DEFAULT_DUMP)
    args = parser.parse_args()

    regex = re.compile(args.pattern)
    shown = 0
    for path in sorted(args.dump.glob("*.lua")):
        if args.only and args.only not in path.name:
            continue
        text = path.read_text("utf-8", "replace")
        for match in regex.finditer(text):
            start = max(0, match.start() - args.window)
            end = min(len(text), match.end() + args.window)
            print(f"{path.name}")
            print(f"  in     : {owner_of(text, match.start())}")
            print(f"  context: ...{text[start:end]}...".replace("\n", " "))
            shown += 1
            if shown >= args.max:
                return 0
            break
    if shown == 0:
        print(f"no match for {args.pattern!r}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
