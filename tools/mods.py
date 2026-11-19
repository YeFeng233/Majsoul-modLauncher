# -*- coding: utf-8 -*-
"""MaJmods 批量开关工具（按 128 MiB 解码预算安全切换）。

用法（数据目录以脚本所在的项目为准）：

    python tools/mods.py list                  # 列出所有 mod：状态、ID、解码占用、动态图集
    python tools/mods.py budget                # 只看预算占用与余量
    python tools/mods.py on  200048            # 按角色 ID / 皮肤 ID / 名称片段启用
    python tools/mods.py off 森川               # 停用匹配项
    python tools/mods.py char 森川绫子          # 只启用该角色的全部皮肤（同角色其它皮肤保持，其它角色不动）
    python tools/mods.py only 森川绫子          # 只保留该角色启用，其它全部停用（最省预算）
    python tools/mods.py preset dynamic        # 启用全部动态(spine)模组，能装几个装几个
    python tools/mods.py reload                # 写 command.txt 触发游戏内热重载（等同 F10）

任何会超过 128 MiB 的操作都会先算出结果并拒绝执行，不做半途修改。
"""
import os
import re
import struct
import sys

sys.stdout.reconfigure(encoding="utf-8")

PROJECT_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BASE = os.path.join(PROJECT_ROOT, "LocalPreview", "MaJmods")
MODS = os.path.join(BASE, "mods")
LIMIT = 128 * 1024 * 1024
SOFT = 122 * 1024 * 1024          # 工具自己的舒适上限，留 6 MiB 余量


def png_bytes(path):
    with open(path, "rb") as fh:
        head = fh.read(33)
    if head[:8] != b"\x89PNG\r\n\x1a\n":
        return 0
    w, h = struct.unpack(">II", head[16:24])
    return w * h * 4


def load():
    items = []
    if not os.path.isdir(MODS):
        return items
    for role in sorted(os.listdir(MODS)):
        rp = os.path.join(MODS, role)
        if not os.path.isdir(rp):
            continue
        for folder in sorted(os.listdir(rp)):
            fp = os.path.join(rp, folder)
            ini = os.path.join(fp, "mod.ini")
            if not os.path.isfile(ini):
                continue
            text = open(ini, encoding="utf-8-sig").read()
            values = {}
            for line in text.splitlines():
                line = line.strip()
                if not line or line[0] in "#;":
                    continue
                if "=" in line:
                    k, v = line.split("=", 1)
                    values[k.strip()] = v.strip()
            char = int(values.get("character_id") or values.get("character") or 0)
            skin = int(values.get("skin_id") or values.get("skin") or 0)
            image = values.get("image", "custom.png")
            ip = os.path.join(fp, image.replace("/", os.sep))
            cost = png_bytes(ip) if os.path.isfile(ip) else 0
            layers = []
            sp = os.path.join(fp, "spine")
            if os.path.isdir(sp):
                for layer in sorted(os.listdir(sp)):
                    lp = os.path.join(sp, layer)
                    if not os.path.isdir(lp):
                        continue
                    for f in sorted(os.listdir(lp)):
                        if f.lower().endswith(".png"):
                            cost += png_bytes(os.path.join(lp, f))
                            layers.append("%s/%s" % (layer, f))
            items.append(dict(role=role, folder=folder, dir=fp, ini=ini, char=char, skin=skin,
                              enabled=values.get("enabled", "1") == "1",
                              cost=cost, spine=bool(layers), text=text))
    return items


def set_enabled(item, on):
    text = item["text"]
    line = "enabled = %d" % (1 if on else 0)
    if re.search(r"^\s*enabled\s*=", text, re.M):
        text = re.sub(r"^\s*enabled\s*=.*$", line, text, count=1, flags=re.M)
    else:
        text = line + "\n" + text
    with open(item["ini"], "w", encoding="utf-8", newline="\n") as fh:
        fh.write(text)
    item["enabled"] = on
    item["text"] = text


def total(items, only=None):
    s = 0
    for it in items:
        if only is not None:
            if it in only:
                s += it["cost"]
        elif it["enabled"]:
            s += it["cost"]
    return s


def check_unique(items):
    seen = {}
    for it in items:
        if not it["enabled"]:
            continue
        key = (it["char"], it["skin"])
        if key in seen:
            print("! 冲突：%s 与 %s 都是 (%d, %d) 且已启用，整批重载会失败" %
                  (seen[key], it["role"] + "/" + it["folder"], it["char"], it["skin"]))
            return False
        seen[key] = it["role"] + "/" + it["folder"]
    return True


def show(items):
    on = [i for i in items if i["enabled"]]
    off = [i for i in items if not i["enabled"]]
    print("%-4s %-8s %-8s %9s  %-10s %s" % ("", "角色ID", "皮肤ID", "解码MiB", "动态", "路径"))
    for it in on + off:
        print("%-4s %-8d %-8d %9.2f  %-10s %s/%s" % (
            "ON" if it["enabled"] else "-", it["char"], it["skin"],
            it["cost"] / 1048576, "spine" if it["spine"] else "", it["role"], it["folder"]))
    t = total(items)
    print("\n启用 %d 个，停用 %d 个；解码总量 %.2f MiB / 128 MiB（余量 %.2f MiB）" % (
        len(on), len(off), t / 1048576, (LIMIT - t) / 1048576))


def match(items, needle):
    needle = needle.strip()
    hits = []
    for it in items:
        keys = [str(it["char"]), str(it["skin"]), it["role"], it["folder"],
                it["role"] + "/" + it["folder"]]
        if any(needle.lower() in k.lower() for k in keys):
            hits.append(it)
    return hits


def apply_plan(items, plan, label):
    """plan: {ini 路径: True/False}；先算总量，超预算就不写。"""
    result = [it for it in items if plan.get(it["ini"], it["enabled"])]
    cost = sum(it["cost"] for it in result)
    if cost > LIMIT:
        print("拒绝：%s 后解码总量 %.2f MiB，超过 128 MiB。先停用别的模组再试。" % (label, cost / 1048576))
        return False
    seen = {}
    for it in result:
        key = (it["char"], it["skin"])
        if key in seen:
            print("拒绝：%s 会让 (%d,%d) 出现两个启用模组（%s 与 %s）。" %
                  (label, it["char"], it["skin"], seen[key], it["role"] + "/" + it["folder"]))
            return False
        seen[key] = it["role"] + "/" + it["folder"]
    for it in items:
        want = plan.get(it["ini"])
        if want is not None and it["enabled"] != want:
            set_enabled(it, want)
    t = sum(i["cost"] for i in items if i["enabled"])
    print("%s 完成；解码总量 %.2f MiB / 128 MiB（余量 %.2f MiB）" % (
        label, t / 1048576, (LIMIT - t) / 1048576))
    if t > SOFT:
        print("提示：已接近 128 MiB 上限，运行期还会额外创建纹理，建议再停用 1-2 个。")
    return True


def main(argv):
    items = load()
    if not items:
        print("没有找到任何 mod：", MODS)
        return 1
    if not argv or argv[0] in ("list", "ls"):
        show(items)
        return 0
    cmd, args = argv[0], argv[1:]

    if cmd == "budget":
        t = total(items)
        print("启用 %d 个 / 共 %d 个；解码 %.2f MiB / 128 MiB（余量 %.2f MiB）" % (
            sum(1 for i in items if i["enabled"]), len(items), t / 1048576, (LIMIT - t) / 1048576))
        print("\n按角色：")
        roles = {}
        for it in items:
            if it["enabled"]:
                roles[it["role"]] = roles.get(it["role"], 0) + it["cost"]
        for role, c in sorted(roles.items(), key=lambda kv: -kv[1]):
            print("  %-20s %7.2f MiB" % (role, c / 1048576))
        return 0

    if cmd == "reload":
        path = os.path.join(BASE, "command.txt")
        with open(path, "w", encoding="utf-8") as fh:
            fh.write("reload")
        print("已写入 %s；游戏在前台时会按 F10 的效果重扫（进程启动过才会生效）。" % path)
        return 0

    if cmd in ("on", "off"):
        if not args:
            print("用法: %s <角色ID|皮肤ID|名称片段>" % cmd)
            return 1
        hits = match(items, args[0])
        if not hits:
            print("没有匹配项:", args[0])
            return 1
        want = cmd == "on"
        return 0 if apply_plan(items, {it["ini"]: want for it in hits}, "%s %s" % (cmd, args[0])) else 1

    if cmd in ("char", "only"):
        if not args:
            print("用法: %s <角色名|角色ID>" % cmd)
            return 1
        hits = match(items, args[0])
        if not hits:
            print("没有匹配项:", args[0])
            return 1
        roles = {it["role"] for it in hits}
        plan = {}
        for it in items:
            if it["role"] in roles:
                plan[it["ini"]] = True
            elif cmd == "only":
                plan[it["ini"]] = False
        return 0 if apply_plan(items, plan, "%s %s" % (cmd, args[0])) else 1

    if cmd == "preset":
        if args and args[0] == "dynamic":
            plan = {it["ini"]: it["spine"] for it in items}
            return 0 if apply_plan(items, plan, "preset dynamic") else 1
        if args and args[0] == "static":
            plan = {it["ini"]: not it["spine"] for it in items}
            return 0 if apply_plan(items, plan, "preset static") else 1
        print("用法: preset dynamic|static")
        return 1

    print(__doc__)
    return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
