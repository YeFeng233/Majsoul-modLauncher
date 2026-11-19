"""Decode Mahjong Soul *.majset file names back to their logical asset paths.

The client ships bundles under names obfuscated by a per-character rotation over a
40-character alphabet (a-z, 0-9, @, $, _, -) with a fixed offset of 17. Decoding the
NAME needs no key and no decryption of the payload, so it enumerates which bundle
holds what - including the Lua module names - without running the game.

Read-only: lists file names, reads no bundle content.
"""
from __future__ import annotations

import string
import sys
from collections import Counter
from pathlib import Path

ALPHABET = string.ascii_lowercase + string.digits + "@$_-"
INDEX = {c: i for i, c in enumerate(ALPHABET)}
N = len(ALPHABET)
OFFSET = 17
SUFFIX_RE_LEN = len("_ba0fc633e65b17bd4b41.majset")  # _<32 hex>.majset


def decrypt(text: str) -> str:
    out = []
    for pos, ch in enumerate(text):
        if ch in INDEX:
            out.append(ALPHABET[(INDEX[ch] - pos - OFFSET) % N])
        else:
            out.append(ch)
    return "".join(out).replace("$@$", "_")


def logical_name(filename: str) -> str | None:
    stem = filename[:-len(".majset")]
    if "_" not in stem:
        return None
    enc, _, digest = stem.rpartition("_")
    # The trailing digest length is build dependent (20 hex chars on 4.0.8 clients).
    if not enc or not (12 <= len(digest) <= 40) or any(c not in string.hexdigits for c in digest):
        return None
    return decrypt(enc)


def main() -> int:
    roots = [Path(p) for p in (sys.argv[1:] or [r"E:\MajSoul"])]
    for root in roots:
        data_dirs = sorted(root.glob("*_Data")) + sorted(root.glob("*/*_Data"))
        for data_dir in data_dirs:
            names: Counter[str] = Counter()
            bad = 0
            for area in ("StreamingAssets", "ABM-Fold"):
                base = data_dir / area
                if not base.is_dir():
                    continue
                for f in base.rglob("*.majset"):
                    n = logical_name(f.name)
                    if n:
                        names[n] += 1
                    else:
                        bad += 1
            if not names and not bad:
                continue
            dump = Path("analysis") / f"majset-names-{data_dir.name}.txt"
            dump.write_text("\n".join(sorted(names)) + "\n", encoding="utf-8")
            print(f"\n===== {data_dir.name}   decoded={sum(names.values())} undecodable={bad}")
            print(f"      distinct logical names={len(names)} -> {dump}")
            groups = Counter()
            for n in names:
                head = n.split("/")[0]
                groups[head] += 1
            print("      top-level groups:", dict(groups.most_common(15)))
            for kw in ("lua", "Lua", "lobby", "Lobby", "bag", "skin", "character", "views", "title"):
                hits = sorted(n for n in names if kw in n)
                if hits:
                    print(f"      [{kw}] {len(hits)} -> {hits[:12]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
