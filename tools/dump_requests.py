"""List every lobby request method the client can send, with call sites.

Used to decide which requests must be answered locally instead of being forwarded:
anything carrying an item/skin/title id we made up locally must never reach the server.

    python tools/dump_requests.py
    python tools/dump_requests.py --filter skin
"""
from __future__ import annotations

import argparse
import collections
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from dump_grep import load_index, pick_dump  # noqa: E402

DIRECT = re.compile(r"LobbyNetMgr\.SendRequest\(\s*'Lobby'\s*,\s*'([A-Za-z_0-9]+)'")
WRAPPED = re.compile(r"SendLobbyRequest\(\s*'([A-Za-z_0-9]+)'")
PROTO = re.compile(r"ProtoMgr\.CreateRequest(?:FromTable)?\(\s*'Lobby'\s*,\s*'([A-Za-z_0-9]+)'")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dump")
    parser.add_argument("--filter", help="substring filter on the method name")
    args = parser.parse_args()

    dump = pick_dump(args.dump)
    if dump is None:
        print("no dump found", file=sys.stderr)
        return 1
    rows = load_index(dump)

    counts: collections.Counter[str] = collections.Counter()
    kinds: dict[str, set[str]] = collections.defaultdict(set)
    where: dict[str, set[str]] = collections.defaultdict(set)
    for file, name, _size in rows:
        if name.startswith("@Excels/"):
            continue
        text = (dump / file).read_text(encoding="utf-8", errors="replace")
        for pattern, kind in ((DIRECT, "send"), (WRAPPED, "sendLobby"), (PROTO, "proto")):
            for match in pattern.finditer(text):
                method = match.group(1)
                counts[method] += 1
                kinds[method].add(kind)
                where[method].add(name)

    selected = [m for m in counts if not args.filter or args.filter.lower() in m.lower()]
    print(f"# {len(counts)} distinct lobby methods, {len(selected)} shown")
    for method in sorted(selected, key=lambda m: -counts[m]):
        sites = ",".join(sorted(where[method])[:2])
        print(f"{counts[method]:>3}  {method:<34} {'+'.join(sorted(kinds[method])):<10} {sites}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
