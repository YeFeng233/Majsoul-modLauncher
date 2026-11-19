"""Read-only probe #4: how does the CN (official) client bind to tolua?

Answers: which managed type wraps the native Lua C API, whether its method names are
obfuscated in this build, and whether any clean 'luaL_loadbuffer' name survives.

Bytes only - no DLL loading, no execution.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from inspect_client import Metadata  # noqa: E402

OBF = re.compile(r"^[A-Z]{8,}$")


def main() -> int:
    target = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(
        r"E:\MajSoul\GameClient\Jantama_MahjongSoul_Data\il2cpp_data\Metadata\global-metadata.dat")
    md = Metadata(target)
    types = md.types()

    print(f"== {target}")
    print(f"   types={len(types)}")

    lua_types = [t for t in types if re.search(r"Lua|CString|ToLua", t["name"], re.I)]
    print(f"\n== Lua-related types ({len(lua_types)})")
    for t in sorted(lua_types, key=lambda x: -len(x["methods"]))[:25]:
        clean = [m["name"] for m in t["methods"] if not OBF.match(m["name"])]
        obf = [m["name"] for m in t["methods"] if OBF.match(m["name"])]
        print(f"   {t['name']:<42} methods={len(t['methods']):<4} clean={clean[:6]} obf={len(obf)}")

    print("\n== 4-param static methods in Lua types (luaL_loadbuffer shape)")
    for t in lua_types:
        for m in t["methods"]:
            if m.get("static") and len(m["parameters"]) == 4:
                print(f"   {t['name']}.{m['name']}  params={m['parameters']}")

    print("\n== any method whose name looks like the C API")
    for t in types:
        for m in t["methods"]:
            if re.search(r"loadbuffer|loadstring|lua_load|tolua_load", m["name"], re.I):
                print(f"   {t['name']}.{m['name']}")

    print("\n== literals mentioning tolua / lua dll / loadbuffer")
    hits = [s for s in md.literals() if re.search(r"^tolua$|luaL_loadbuffer|tolua_loadbuffer|LuaDLL|LuaInterface", s, re.I)]
    for s in sorted(set(hits))[:40]:
        print(f"   {s!r}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
