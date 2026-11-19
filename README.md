# Majsoul-modLauncher

当前启动器版本为 **0.6.9**，内置 129 位角色头像及 526 款皮肤资料，包含泠星与《葬送的芙莉莲》联动角色。

0.6.9 补齐本人个人名片的立绘替换，支持静态立绘、动态图集、多实例窗口以及 F10/F11 更新与恢复。保留名片原有遮罩和布局。

0.6.8 修复导入 Mod 的中文头像文件名识别，直接支持 `小头像.png` / `大头像.png`；同类型英文文件 `smallhead.png` / `bighead.png` 优先。更新 DLL 后通过新版启动器重新启动游戏即可启用。

0.6.7 修复 Steam 版启动后重新拉起游戏、丢失本地预览的问题。根据 Steam 安装记录设置游戏子进程的应用环境，启动时确认游戏进程持续运行，再显示成功。使用 Steam 版时请先登录 Steam。

从 [GitHub Releases](https://github.com/YeFeng233/Majsoul-modLauncher/releases) 下载 Windows x86 压缩包，完整解压后运行 `Majsoul-modLauncher.exe`。`MaJmods.dll` 和 `MaJmods` 数据目录需与 EXE 放在同一目录。升级时，把新版 EXE 和 DLL 放入原安装目录。若新位置尚未存在，首次运行会把旧数据目录和游戏路径配置分别迁移至 `MaJmods` 和 `MaJmods.ini`。更新物品目录时，将 [ID 清单](LocalPreview/data/max_data.yaml)放入 `MaJmods` 子目录。[使用说明](LocalPreview/README.txt)。

制作或调整自定义立绘请参阅 [Mod 适配指南](MOD适配指南.md)，包含目录结构、角色与皮肤 ID、两个示例，以及图片尺寸和各场景显示比例的调整方法。

角色一览支持按名称/ID 搜索、已配置筛选和 Mod 数量显示，顶部可一键启用或关闭所有角色的 Mod。点击角色可单独启停 Mod、替换图片、导入指定皮肤的新图片。启动器记住游戏路径、显示进度，并可在成功后自动关闭。F10 重载与 F11 恢复保留。

两个示例 Mod 均为二之宫花（200017）的皮肤。默认皮肤 401701 和纯白午后 401710 分别存放在 `MaJmods/mods/二之宫花/默认替换/` 和 `MaJmods/mods/二之宫花/纯白午后/`。原图尺寸、位置、缩放与裁切对齐继续使用。

`LocalPreview/gui` 为图形界面与内置头像资料，`src`、`data`、`tests` 为原生代码、默认数据及验证；`build.ps1` 编译图形启动器和 DLL，`verify.ps1` 进行原生功能验证。`tools` 保留客户端分析工具。[开发说明](LocalPreview/README.md)。
