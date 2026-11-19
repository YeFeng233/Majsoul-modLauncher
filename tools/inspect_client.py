"""Read-only Mahjong Soul client inspection. Python 3.10+, standard library only.

Does not load DLLs, execute extracted code, contact servers, or change game files.
Metadata layouts: https://github.com/Perfare/Il2CppDumper/blob/master/Il2CppDumper/Il2Cpp/MetadataClass.cs
"""
from __future__ import annotations

import argparse
import base64
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import struct


def sha256(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def read_catalog(path: Path) -> dict:
    """Deliberately accept only this catalog's mapping/integer-list YAML subset.

    No arbitrary YAML tags, aliases, object construction, or executable content.
    """
    result = {}
    section = None
    owner = None
    for line_number, raw in enumerate(path.read_text(encoding="utf-8-sig").splitlines(), 1):
        line = raw.split("#", 1)[0].rstrip()
        if not line:
            continue
        section_match = re.fullmatch(r"([a-z_]+):", line)
        owner_match = re.fullmatch(r"  ([0-9]+):", line)
        entry_match = re.fullmatch(r"( *)- ([0-9]+)", line)
        error = f"{path.name}:{line_number}: unsupported or invalid catalog syntax"
        if section_match:
            section = section_match[1]
            if section not in {"character", "skin", "title", "item", "loading_image", "emoji", "endings"}:
                raise ValueError(error)
            if section in result:
                raise ValueError(f"{error} (duplicate section)")
            result[section] = {} if section == "emoji" else []
            owner = None
        elif owner_match and section == "emoji":
            owner = int(owner_match[1])
            if owner <= 0 or owner in result[section]:
                raise ValueError(f"{error} (invalid or duplicate character ID)")
            result[section][owner] = []
        elif entry_match and section:
            item_id = int(entry_match[2])
            if section == "emoji":
                if len(entry_match[1]) != 2 or owner is None:
                    raise ValueError(error)
                target = result[section][owner]
            else:
                if entry_match[1]:
                    raise ValueError(error)
                target = result[section]
            if item_id < 0 or item_id > 0x7FFFFFFF or (item_id == 0 and section != "emoji"):
                raise ValueError(f"{error} (ID out of range)")
            if item_id in target:
                raise ValueError(f"{error} (duplicate ID {item_id})")
            target.append(item_id)
        else:
            raise ValueError(error)
    if not result:
        raise ValueError("Catalog is empty")
    return result


def catalog_summary(catalog: dict) -> dict:
    summary = {}
    for section, values in catalog.items():
        if isinstance(values, dict):
            summary[section] = {
                "characters": len(values),
                "character_expression_pairs": sum(map(len, values.values())),
                "unknown_character_ids": sorted(set(values) - set(catalog.get("character", []))),
            }
        else:
            summary[section] = {"count": len(values), "min": min(values, default=None), "max": max(values, default=None)}
    return summary


class Metadata:
    def __init__(self, path: Path):
        self.data = path.read_bytes()
        magic, self.version = struct.unpack_from("<II", self.data)
        if magic != 0xFAB11BAF or self.version not in {27, 29, 31}:
            raise ValueError("Unsupported metadata header/version")
        self.strings = self.table(2)

    def table(self, index: int) -> bytes:
        offset, size = struct.unpack_from("<II", self.data, 8 + index * 8)
        if offset + size > len(self.data):
            raise ValueError("Metadata table outside file")
        return self.data[offset:offset + size]

    def name(self, index: int) -> str:
        if not 0 <= index < len(self.strings):
            raise ValueError("Invalid metadata string index")
        return self.strings[index:self.strings.index(0, index)].decode("utf-8")

    def literals(self):
        data = self.table(1)
        for size, offset in struct.iter_unpack("<II", self.table(0)):
            if offset + size > len(data):
                raise ValueError("Invalid literal range")
            yield data[offset:offset + size].decode("utf-8", errors="replace")

    def types(self) -> list[dict]:
        types = self.table(19)
        methods = self.table(5)
        fields = self.table(11)
        parameters = self.table(10)
        method_size = 36 if self.version >= 31 else 32
        if len(types) % 88 or len(methods) % method_size:
            raise ValueError("Unsupported metadata table layout")
        rows = []
        for idx, offset in enumerate(range(0, len(types), 88)):
            row = struct.unpack_from("<16i8H2I", types, offset)
            ns, name = self.name(row[1]), self.name(row[0])
            entries = []
            for method_index in range(row[9], row[9] + row[16]):
                pos = method_index * method_size
                method_name, declaring_type = struct.unpack_from("<Ii", methods, pos)
                if declaring_type != idx:
                    raise ValueError("Method owner does not match type table")
                parameter_start = struct.unpack_from("<i", methods, pos + (16 if self.version >= 31 else 12))[0]
                flags, _, _, parameter_count = struct.unpack_from("<4H", methods, pos + method_size - 8)
                parameter_names = [self.name(struct.unpack_from("<I", parameters, 12 * i)[0]) for i in range(parameter_start, parameter_start + parameter_count)]
                entries.append({"name": self.name(method_name), "parameters": parameter_names, "static": bool(flags & 0x10)})
            field_names = [self.name(struct.unpack_from("<I", fields, 12 * i)[0]) for i in range(row[8], row[8] + row[18])]
            rows.append({"name": f"{ns}.{name}" if ns else name, "methods": entries, "fields": field_names})
        return rows


def pe_info(path: Path) -> dict:
    data = path.read_bytes()
    if data[:2] != b"MZ":
        raise ValueError("Not a PE file")
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError("Invalid PE signature")
    machine, sections = struct.unpack_from("<HH", data, pe + 4)
    optional_size = struct.unpack_from("<H", data, pe + 20)[0]
    optional = pe + 24
    magic = struct.unpack_from("<H", data, optional)[0]
    directory = optional + (96 if magic == 0x10B else 112)
    section_table = optional + optional_size

    def position(rva: int) -> int:
        for i in range(sections):
            vs, va, rs, rp = struct.unpack_from("<IIII", data, section_table + i * 40 + 8)
            if va <= rva < va + max(vs, rs):
                return rp + rva - va
        raise ValueError("PE RVA outside sections")

    exports = []
    export_rva = struct.unpack_from("<I", data, directory)[0]
    if export_rva:
        exp = position(export_rva)
        count = struct.unpack_from("<I", data, exp + 24)[0]
        names = position(struct.unpack_from("<I", data, exp + 32)[0])
        for i in range(count):
            p = position(struct.unpack_from("<I", data, names + i * 4)[0])
            exports.append(data[p:data.index(0, p)].decode("ascii"))
    return {"architecture": {0x14C: "x86", 0x8664: "x64"}.get(machine, hex(machine)),
            "managed_clr_header": bool(struct.unpack_from("<I", data, directory + 14 * 8)[0]),
            "export_count": len(exports), "il2cpp_exports": [x for x in exports if x.startswith("il2cpp_")],
            "lua_exports": [x for x in exports if re.match(r"(?:lua|tolua|luaL)_", x)]}


class Reader:
    def __init__(self, data: bytes):
        self.data, self.pos = data, 0

    def take(self, count: int) -> bytes:
        if count < 0 or self.pos + count > len(self.data):
            raise ValueError("Unexpected end of binary data")
        result = self.data[self.pos:self.pos + count]
        self.pos += count
        return result

    def unpack(self, fmt: str):
        return struct.unpack(fmt, self.take(struct.calcsize(fmt)))

    def string(self) -> str:
        end = self.data.index(0, self.pos)
        return self.take(end + 1 - self.pos)[:-1].decode("utf-8")

    def align(self, size=16):
        self.take((-self.pos) % size)


def lz4_block(data: bytes, expected: int) -> bytes:
    """Decode the raw LZ4 blocks used by UnityFS; enforce output bounds."""
    r, output = Reader(data), bytearray()

    def length(value):
        if value == 15:
            while True:
                extra = r.take(1)[0]
                value += extra
                if extra != 255:
                    break
        return value

    while r.pos < len(data):
        token = r.take(1)[0]
        literal_count = length(token >> 4)
        if len(output) + literal_count > expected:
            raise ValueError("LZ4 literal exceeds declared size")
        output.extend(r.take(literal_count))
        if r.pos == len(data):
            break
        distance = r.unpack("<H")[0]
        match_count = length(token & 15) + 4
        if distance == 0 or distance > len(output) or len(output) + match_count > expected:
            raise ValueError("Invalid LZ4 match")
        chunk = bytes(output[-distance:])
        output.extend((chunk * ((match_count + distance - 1) // distance))[:match_count])
    if len(output) != expected:
        raise ValueError("LZ4 decoded size mismatch")
    return bytes(output)


def decompress(data: bytes, expected: int, flags: int) -> bytes:
    kind = flags & 0x3F
    if expected > 512 * 1024 * 1024:
        raise ValueError("Block exceeds inspection memory limit")
    if kind in (2, 3):
        return lz4_block(data, expected)
    if kind == 0 and len(data) == expected:
        return data
    raise ValueError(f"Unsupported UnityFS compression: {kind}")


def bundle_contents(path: Path, payload=False, prefix_only=False) -> tuple[dict, list[tuple[str, bytes]]]:
    r = Reader(path.read_bytes())
    if r.string() != "UnityFS":
        raise ValueError("Not a UnityFS bundle")
    version = r.unpack(">I")[0]
    unity_version, generator = r.string(), r.string()
    file_size, compressed, uncompressed, flags = r.unpack(">QIII")
    if file_size != len(r.data):
        raise ValueError("UnityFS file size mismatch")
    if version >= 7:
        r.align()
    if flags & 0x80:
        info_data = r.data[-compressed:]
    else:
        info_data = r.take(compressed)
    info = Reader(decompress(info_data, uncompressed, flags))
    info.take(16)
    block_count = info.unpack(">I")[0]
    blocks = [info.unpack(">IIH") for _ in range(block_count)]
    node_count = info.unpack(">I")[0]
    nodes = []
    for _ in range(node_count):
        offset, size, node_flags = info.unpack(">QQI")
        nodes.append({"offset": offset, "size": size, "flags": node_flags, "name": info.string()})
    if info.pos != len(info.data):
        raise ValueError("Unrecognized UnityFS directory data")
    if flags & 0x200:
        r.align()
    result = {"file": path.name, "size": file_size, "format": version, "unity_version": unity_version,
              "generator": generator, "flags": hex(flags), "blocks": block_count, "nodes": nodes}
    files = []
    if payload:
        if sum(b[0] for b in blocks) > 512 * 1024 * 1024:
            raise ValueError("Bundle exceeds inspection memory limit")
        chunks = []
        for u, c, f in blocks:
            chunks.append(decompress(r.take(c), u, f))
            if prefix_only:
                break
        decoded = b"".join(chunks)
        for node in nodes:
            begin, end = node["offset"], node["offset"] + node["size"]
            if prefix_only:
                if begin < len(decoded):
                    files.append((node["name"], decoded[begin:min(end, len(decoded))]))
                continue
            if end > len(decoded):
                raise ValueError("Bundle node outside decoded data")
            files.append((node["name"], decoded[begin:end]))
    return result, files


def serialized_objects(blob: bytes) -> list[tuple[int, int, bytes]]:
    """Read object boundaries (not execute/deserialise MonoBehaviour scripts)."""
    r = Reader(blob)
    _, size, version, offset = r.unpack(">4I")
    if version not in (17, 18, 19, 20, 21, 22):
        raise ValueError("Unsupported serialized file version")
    endian = r.take(4)[0]
    if version >= 22:
        _, size, offset, _ = r.unpack(">IQQQ")
    if size != len(blob):
        raise ValueError("Incomplete serialized file")
    e = "<" if endian == 0 else ">"
    r.string()
    r.unpack(e + "i")
    tree = r.take(1)[0]
    count = r.unpack(e + "i")[0]
    types = []
    for _ in range(count):
        class_id = r.unpack(e + "i")[0]
        r.take(1)
        r.unpack(e + "h")
        if class_id == 114:
            r.take(16)
        r.take(16)
        if tree:
            n, s = r.unpack(e + "ii")
            r.take(n * (32 if version >= 19 else 24) + s)
            if version >= 21:
                r.take(r.unpack(e + "i")[0] * 4)
        types.append(class_id)
    result = []
    for _ in range(r.unpack(e + "i")[0]):
        r.align(4)
        path_id = r.unpack(e + "q")[0]
        start = r.unpack(e + ("q" if version >= 22 else "I"))[0]
        size, type_index = r.unpack(e + "Ii")
        if offset + start + size > len(blob) or start < 0 or not 0 <= type_index < len(types):
            raise ValueError("Invalid serialized object range")
        result.append((path_id, types[type_index], blob[offset + start:offset + start + size]))
    return result


def text_assets(blob: bytes):
    # This client's serialized files are little endian.
    if blob[16] != 0:
        raise ValueError("TextAsset reader requires little endian data")
    for path_id, class_id, data in serialized_objects(blob):
        if class_id == 49:
            r = Reader(data)
            name = r.take(r.unpack("<i")[0]).decode("utf-8")
            r.align(4)
            script = r.take(r.unpack("<i")[0])
            yield path_id, name, script


def inspect(game: Path, catalog_path: Path, output: Path, scan_payloads=False, scan_prefixes=False, include_cache=False):
    game, output = game.resolve(), output.resolve()
    if output == game or game in output.parents:
        raise ValueError("Output must be outside the game installation")
    catalog = read_catalog(catalog_path)
    report = {"game_root": str(game), "catalog": {"path": str(catalog_path), "sha256": sha256(catalog_path), "sections": catalog_summary(catalog)},
              "programs": [], "bundles": [], "errors": []}
    for dll in [game / "GameAssembly.dll", game / "GameClient/GameAssembly.dll"]:
        if not dll.exists():
            continue
        data_folders = [p for p in dll.parent.glob("*_Data") if (p / "il2cpp_data/Metadata/global-metadata.dat").is_file()]
        for data in data_folders:
            metadata_path = data / "il2cpp_data/Metadata/global-metadata.dat"
            metadata = Metadata(metadata_path)
            types = metadata.types()
            candidates = [row for row in types if re.fullmatch(r"(?:LuaClient|LuaResLoader|LuaManager|LuaTools|ResLoadMgr|LuaInterface\.(?:LuaState|LuaFileUtils|LuaDLL)|AssetBundles\.(?:ABTools|PersistentMgr|ResManager|AssetBundleManager|AssetBundleConfigData)|.*(?:AssetLoader|AssetManager|ResourceManager|EncryptUtil|EncryptHelper|FileUtils))", row["name"])]
            literals = [s for s in metadata.literals() if len(s) < 240 and re.search(r"Lua/|\.lua$|\.majset|AssetBundleConfig|persistentDataPath|Config/|cache|character|skin", s, re.I)]
            report["programs"].append({"path": str(dll), "sha256": sha256(dll), "pe": pe_info(dll),
                "unity_versions": sorted(set(x.decode() for x in re.findall(rb"20\d\d\.\d+\.\d+[abfp]\d+", (data / "globalgamemanagers").read_bytes()))),
                "metadata": {"version": metadata.version, "sha256": sha256(metadata_path), "type_count": len(types), "candidate_types": candidates, "resource_literals": literals},
                "client_version": (data / "version.txt").read_text().strip() if (data / "version.txt").exists() else None})
            config = data / "ABM-CONFIG/StandaloneWindows/AssetBundleConfig.json"
            if config.exists():
                raw = config.read_bytes()
                decoded = base64.b64decode(raw, validate=True)
                report["asset_index"] = {"path": str(config), "size": len(raw), "base64_decoded_size": len(decoded), "decoded_prefix_hex": decoded[:16].hex(), "plain_json": raw.lstrip().startswith((b"{", b"["))}
            paths = list((data / "StreamingAssets").rglob("*.majset"))
            if include_cache:
                paths.extend((data / "ABM-Fold").rglob("*.majset"))
            for index, path in enumerate(sorted(paths)):
                try:
                    summary, files = bundle_contents(path, scan_payloads or scan_prefixes, scan_prefixes and not scan_payloads)
                    summary["relative_path"] = str(path.relative_to(game))
                    summary["payload_scan"] = "full" if scan_payloads else "first_block" if scan_prefixes else "none"
                    if files:
                        summary["readable_paths"] = sorted(set(s.decode("ascii") for _, blob in files for s in re.findall(rb"[A-Za-z0-9_./-]{5,160}\.(?:lua|bytes|prefab|png|jpg|asset|atlas|json|skel)", blob)))[:80]
                    report["bundles"].append(summary)
                except (ValueError, struct.error, UnicodeError) as exc:
                    report["errors"].append({"path": str(path), "error": str(exc)})
                if (index + 1) % 1000 == 0:
                    print(f"Inspected {index + 1}/{len(paths)} bundles", flush=True)
    report["bundle_summary"] = {"count": len(report["bundles"]), "total_bytes": sum(x["size"] for x in report["bundles"]), "formats": dict(Counter(x["format"] for x in report["bundles"]))}
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"report": str(output), "catalog": report["catalog"]["sections"], "programs": [{"path": x["path"], "architecture": x["pe"]["architecture"], "unity": x["unity_versions"], "metadata_version": x["metadata"]["version"]} for x in report["programs"]], "bundles": report["bundle_summary"], "errors": len(report["errors"])}, ensure_ascii=False))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game", type=Path, default=Path(r"E:\MajSoul"))
    parser.add_argument("--catalog", type=Path, default=Path(__file__).resolve().parents[1] / "LocalPreview/data/max_data.yaml")
    parser.add_argument("--output", type=Path, default=Path("analysis/client-report.json"))
    parser.add_argument("--scan-payloads", action="store_true", help="Also decompress bundled data and list readable resource paths")
    parser.add_argument("--scan-prefixes", action="store_true", help="Inspect only the first compressed block of each bundle")
    parser.add_argument("--include-cache", action="store_true", help="Include the installed client's ABM-Fold download directory")
    args = parser.parse_args()
    inspect(args.game, args.catalog, args.output, args.scan_payloads, args.scan_prefixes, args.include_cache)
