#!/usr/bin/env python3
"""Grep the Lua module dump and print hits **with surrounding context**.

Unlike dump_grep.py (line based), this works on the raw text: the client's Lua is
minified, so a "line" is often the whole file. Every hit prints a byte window around the
match, which is what you need to read a real function body.

    python tools/lua_context.py PATTERN [PATTERN...] [--window 90] [--max 12]
                                       [--dump DIR] [--only SUBSTRING]
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

DEFAULT_DUMP = Path(r"E:\MajSoul\GameClient\MajsoulPreview\dump")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("patterns", nargs="+")
    parser.add_argument("--window", type=int, default=90, help="bytes of context each side")
    parser.add_argument("--max", type=int, default=12, help="max hits per pattern")
    parser.add_argument("--dump", type=Path, default=DEFAULT_DUMP)
    parser.add_argument("--only", default="", help="restrict to module names containing this")
    args = parser.parse_args()

    files = sorted(args.dump.glob("*.lua"))
    if not files:
        print(f"no .lua files under {args.dump}", file=sys.stderr)
        return 2

    texts: list[tuple[str, str]] = []
    for path in files:
        if args.only and args.only not in path.name:
            continue
        try:
            texts.append((path.name, path.read_text("utf-8", "replace")))
        except OSError:
            continue

    for pattern in args.patterns:
        try:
            regex = re.compile(pattern)
        except re.error as error:
            print(f"bad regex {pattern!r}: {error}", file=sys.stderr)
            return 2
        print(f"===== {pattern}")
        shown = 0
        for name, text in texts:
            for match in regex.finditer(text):
                start = max(0, match.start() - args.window)
                end = min(len(text), match.end() + args.window)
                snippet = text[start:end].replace("\n", " ")
                print(f"  {name}: …{snippet}…")
                shown += 1
                if shown >= args.max:
                    break
            if shown >= args.max:
                break
        if shown == 0:
            print("  (no hits)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
