"""Build the portable launcher catalog from a local Majsoul_Skins directory."""
import argparse
import io
import re
import zipfile
from pathlib import Path
from PIL import Image

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("source", type=Path)
parser.add_argument("--output", type=Path, default=Path(__file__).with_name("assets.zip"))
args = parser.parse_args()
characters, skins, avatars = [], [], []
for role in sorted(args.source.iterdir()):
    match = re.fullmatch(r"(.+)（(\d+)）", role.name)
    if not role.is_dir() or not match or match[2] == "0":
        continue
    role_id, name, rows = int(match[2]), match[1], []
    for folder in role.iterdir():
        skin = re.fullmatch(r"(.+)（(\d+)）", folder.name)
        if folder.is_dir() and skin:
            rows.append((int(skin[2]), skin[1], folder))
    rows.sort()
    if not rows:
        continue
    default = next((r for r in rows if r[1] == name), rows[0])
    picture = default[2] / "小头像.png"
    if not picture.is_file():
        picture = next(role.glob("*/小头像.png"), None)
    if picture:
        with Image.open(picture) as image:
            image = image.convert("RGBA")
            image.thumbnail((192, 192), Image.Resampling.LANCZOS)
            image = image.quantize(colors=256, method=Image.Quantize.FASTOCTREE)
            buffer = io.BytesIO()
            image.save(buffer, format="PNG", optimize=True)
            avatars.append((role_id, buffer.getvalue()))
    characters.append(f"{role_id}\t{name}\t{default[0]}")
    skins.extend(f"{role_id}\t{sid}\t{skin_name}" for sid, skin_name, _ in rows)
with zipfile.ZipFile(args.output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
    archive.writestr("characters.tsv", "\n".join(characters))
    archive.writestr("skins.tsv", "\n".join(skins))
    for role_id, data in avatars:
        archive.writestr(f"avatars/{role_id}.png", data)
print(f"Embedded {len(characters)} characters and {len(skins)} skins.")
