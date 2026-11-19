"""Search a MajsoulPreview Lua dump without needing the sources to be readable.

The client ships its Lua minified (a 62 KB module can be a single line), so line-based
grep is useless. This resolves a chunk name through dump/index.tsv and prints matched
regions with surrounding context.

    python tools/dump_grep.py --list
    python tools/dump_grep.py --module @GameUtility --find "function GameUtility.item_owned"
    python tools/dump_grep.py --module GameMgr --find "skin_map" --context 300

Read-only: reads the dump the injector wrote.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

DEFAULT_DUMPS = [
    Path(r"E:\MajSoul\GameClient\MajsoulPreview\dump"),
    Path(r"E:\MajSoul\MajsoulPreview\dump"),
]


def load_index(dump: Path) -> list[tuple[str, str, int]]:
    """[(file, chunk name, size)] from index.tsv."""
    rows: list[tuple[str, str, int]] = []
    index = dump / "index.tsv"
    if not index.is_file():
        return rows
    for line in index.read_text(encoding="utf-8", errors="replace").splitlines():
        parts = line.split("\t")
        if len(parts) >= 5:
            rows.append((parts[0], parts[4], int(parts[1]) if parts[1].isdigit() else 0))
    return rows


def pick_dump(explicit: str | None) -> Path | None:
    if explicit:
        path = Path(explicit)
        return path if path.is_dir() else None
    for candidate in DEFAULT_DUMPS:
        if (candidate / "index.tsv").is_file():
            return candidate
    return None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dump", help="dump directory (defaults to the first that exists)")
    parser.add_argument("--list", action="store_true", help="list chunk names")
    parser.add_argument("--filter", help="substring filter for --list")
    parser.add_argument("--module", help="chunk name (exact, else substring)")
    parser.add_argument("--global-search", dest="global_search",
                        help="regular expression searched across every chunk")
    parser.add_argument("--find", help="regular expression to locate inside the module")
    parser.add_argument("--context", type=int, default=260, help="characters shown around a match")
    parser.add_argument("--max", type=int, default=6, help="maximum matches to show")
    args = parser.parse_args()

    dump = pick_dump(args.dump)
    if dump is None:
        print("no dump found", file=sys.stderr)
        return 1
    rows = load_index(dump)
    if not rows:
        print(f"{dump}\\index.tsv is empty or missing", file=sys.stderr)
        return 1
    print(f"# dump: {dump}  ({len(rows)} chunks)")

    if args.global_search:
        pattern = re.compile(args.global_search)
        total = 0
        for file, name, size in sorted(rows, key=lambda row: -row[2]):
            if name.startswith("@Excels/"):
                continue  # pure data tables: huge and never hold call sites
            text = (dump / file).read_text(encoding="utf-8", errors="replace")
            hits = list(pattern.finditer(text))
            if not hits:
                continue
            total += len(hits)
            print(f"\n===== {name}  ({size} bytes)  {len(hits)} hit(s)")
            for hit in hits[: args.max]:
                start = max(0, hit.start() - args.context // 2)
                end = min(len(text), hit.end() + args.context)
                print("  " + text[start:end].replace("\n", "\n  "))
        print(f"\n# {total} hit(s) in total")
        return 0

    if args.list or not args.module:
        selected = rows
        if args.filter:
            selected = [row for row in rows if args.filter.lower() in row[1].lower()]
        for file, name, size in sorted(selected, key=lambda row: -row[2]):
            print(f"{size:>8}  {name}")
        return 0

    exact = [row for row in rows if row[1] == args.module]
    matches = exact or [row for row in rows if args.module.lower() in row[1].lower()]
    if not matches:
        print(f"no chunk matching {args.module!r}", file=sys.stderr)
        return 1
    if not args.find:
        for file, name, size in matches:
            print(f"{size:>8}  {name}   -> {dump / file}")
        return 0

    pattern = re.compile(args.find)
    for file, name, size in matches:
        text = (dump / file).read_text(encoding="utf-8", errors="replace")
        print(f"\n===== {name}  ({size} bytes, file {file})")
        count = 0
        for hit in pattern.finditer(text):
            count += 1
            if count > args.max:
                print(f"  ... ({count - 1} of more matches shown)")
                break
            start = max(0, hit.start() - args.context // 2)
            end = min(len(text), hit.end() + args.context)
            print(f"  --- match at {hit.start()} ---")
            print("  " + text[start:end].replace("\n", "\n  "))
        if count == 0:
            print("  (no match)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
