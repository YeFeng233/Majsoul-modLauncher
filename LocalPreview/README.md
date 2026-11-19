# Majsoul-modLauncher 0.6.7

0.6.7 修复 Steam 版的启动环境。通过对应的 appmanifest 核对安装目录和应用 ID，为游戏子进程设置 SteamAppId、SteamGameId。Lua 接管后追加运行状态检查，游戏退出或重开时显示失败。Steam 版需先登录 Steam。

0.6.6 增加游戏内大小头像替换。每个 Mod 根目录可选放入 `smallhead.png` 和 `bighead.png`，启动器 Mod 卡片新增“更换小头像”和“更换大头像”。按原有角色、皮肤 ID 匹配，缺少的头像继续使用原图。F10 重载头像，F11 恢复原头像，异步加载与控件复用会同步处理。头像也计入 1024 MiB 解码预算。通过头像入口显示同一皮肤的本机头像会使用替换图。

0.6.5 将所有启用模组的主图片与动态图集合计解码预算提高至 1024 MiB。配套更新 EXE 和 DLL，下次通过新版启动器启动游戏时生效。Unity 纹理及重载期间新旧集合还会额外占用内存，32 位进程的实际加载量取决于可用地址空间。

制作保留原动画的贴图模组，见 [动态皮肤模组制作指南](../动态皮肤模组制作指南.md)。

0.6.4 将替换接入皮肤加载完成回调和重新显示入口，在同一帧返回渲染器之前绑定自定义纹理，消除等待轮询造成的原皮肤闪现。动态图集仍复用客户端加载的骨骼与动画。保留 80 毫秒轮询用于布局跟随和遗漏实例补查，加载取消时不执行替换，F10/F11 行为不变。新增首次显示、动态 loadingSkin 时序、重新显示和取消回调参数回归检查。

0.6.3 修复动态图集层号与渲染绑定。Mod 的 `spine/layerN/原图集名.png` 优先按原纹理名称及尺寸匹配，层号用于消歧，支持素材 layer2 对应运行时第 1 层。单实例设置 `SkeletonGraphic.OverrideTexture`，多 CanvasRenderer 同时设置 `CustomTextureOverride`，覆盖单子网格与多子网格渲染路径。静态视图继续使用 `image` 图片，缺少匹配图集的动态视图保留原动画。每层支持一张图集，所有图集计入当前版本的 1024 MiB 解码上限。F11、换皮肤和重载会恢复两种覆盖的原值并释放自定义纹理。特效渲染器暂时保留原显示。

本次验证包括 14 项 Rust 测试、957 个模块、17 个适配点及 Lua 图集层号映射、双路径覆盖、已有字典项恢复和组件替换回归。实机已确认悸动之夏的人物图集替换生效，动画继续运行，F11 恢复原贴图、F10 再次启用，服饰预览返回宿舍后仍正确替换。本次未重新实测房间与对局。

启动器由同目录的 `Majsoul-modLauncher.exe` 和 `MaJmods.dll` 组成，使用说明见 [README.txt](README.txt)。从 [GitHub Releases](https://github.com/YeFeng233/Majsoul-modLauncher/releases) 下载并完整解压 Windows x86 发行包。游戏路径保存在同目录的 `MaJmods.ini`。升级时若只有 `MJLocalPreview.ini`，启动器会迁移其中已保存的路径。`--pick` 重新选择，`--game <路径>` 显式指定，`--close-after 0` 保留启动窗口。

## 自定义立绘

每个 Mod 的配置位于 `MaJmods/mods/角色名称/mod名称/mod.ini`，PNG 放在对应的 Mod 文件夹中。文件夹名称只用于整理，按 `character_id`、`skin_id` 匹配，兼容旧字段 `character`、`skin`。交付的两个 Mod 分别指定角色 200017 的默认皮肤 401701 和纯白午后 401710。皮肤 ID 专用配置优先于角色通用配置。将 `skin_id` 设为 0 或省略表示角色通用。同一匹配组合重复启用会拒绝重载。

F10 在原生工作线程使用 Unicode 文件接口重新扫描配置、解码 PNG，全部成功才提交不可变快照，Lua 在游戏线程通过原生回调读取纹理字节。新纹理全部创建成功后才释放旧状态；失败保留旧图。F11 恢复原组件并释放纹理，本次会话保持关闭到 F10 或下次启动。图片尺寸上限 4096×8192，全部图片解码后合计上限 1024 MiB，不生成 RGBA 磁盘缓存。

覆盖大厅、宿舍大图及服饰预览、友人房本人立绘、实际对局本人演出与结算，按本人账号限定，牌谱/观战/教程不处理。静态 PNG 不改造头像、骨骼动画或 3D 模型。角色和皮肤选择按账号保存在游戏 PlayerPrefs，键名前缀 `MJLocalPreview.appearance.v1.`。

## 布局

复制已加载原图的父级、anchors、pivot、localScale（含翻转）、localRotation 和 anchoredPosition，以原图实际高度及自定义图片宽高比确定尺寸。保留服饰轮播缩放、原遮罩与裁切关系，每次更新跟随原图几何变化，正确处理 stretch anchors。动态皮肤通过游戏原有 `Tools.GetIllustDataByType/GetSkinData` 获取布局。无需指定固定布局；各 mod 可选填 `lobby/dorm/dorm_preview/room/match/ending` 的 `_height`、`_x`、`_y`，高度为原图局部单位，位置为相对原图的偏移。

## 启动与数据

启动器校验游戏及同目录 DLL，以挂起状态创建游戏并显式加载 DLL；收到准备事件后恢复主线程，再等待 Lua 接管事件。已运行的游戏需要先退出。普通启动游戏可停止使用工具，注入版不新装游戏目录的 version.dll。

旧版自动加载 DLL 仅在安装记录哈希一致时迁移到备份；普通原始 DLL 会恢复，未知文件保持原样并显示原因。原游戏备份不在本次工作目录清理范围内。新 mods 目录首次生成时仍兼容复制旧 CustomSkins 的图片与配置，已有 mods 不覆盖。

默认目录、运行脚本、适配文件、示例图片均内置在 DLL，运行时使用 `MaJmods/`。如果升级时只有旧的 `MJLocalPreview/` 数据目录，程序会将整个目录改名为 `MaJmods/`。外部 `max_data.yaml`、`runtime.lua`、`adapter.tsv`、`config.ini` 保留用户修改。只增加物品时更新 `max_data.yaml` 后重启即可。Lua 接口变化时适配脚本，原生接口或位数变化时更新加载器。日志为 `preview-进程ID.log` 和 `custom-skin-进程ID.log`。

## 开发与验证

`src/`、`data/`、`tests/` 为当前源码、内置数据与断言，`../tools/` 为客户端分析工具。需要 Rust、i686-pc-windows-msvc、MSVC 和 Windows SDK，无第三方 Rust 依赖，静态链接运行库。

`build.ps1` 编译正式 EXE/DLL；`verify.ps1` 验证。默认使用本机客户端 `E:/MajSoul/GameClient/Jantama_MahjongSoul_Data/Plugins/x86/tolua.dll` 与 `E:/MajSoul/GameClient/MajsoulPreview/dump`，可用 `-Tolua`、`-Dump` 改写。验证只修改私有测试副本。

当前验证覆盖 13 项单元测试、957 个模块语法、17 个唯一适配点、真实客户端函数调用、账号/角色/皮肤隔离、持久保存、原生接管与畸形补丁回退，以及中文目录、mod 冲突、坏图保留旧状态、场景和组件生命周期、F11 恢复、范围收窄、布局位置/尺寸/旋转/翻转/拉伸锚点等。

实机已对照默认皮肤与纯白午后的宿舍大图、服饰预览开关效果，并核对默认皮肤大厅显示，尺寸和裁切已接近原图。自动化按键受前台检查限制时，使用相同原生分发入口对比；保留用户已验证的物理 F10/F11 实现。此前房间和对局组件已实测，0.5.2 布局修改后未重新运行完整实机对局，相关布局通过离线场景验证。

旧独立 MJCustomSkin 原型、对应构建入口和旧版本产物已从工作目录移除，正式构建只使用已整合的 custom_skin 实现。


## 0.6.0 图形启动器

`gui/Launcher.cs` 与 `gui/MainWindow.xaml` 为 WPF 界面，使用 Windows 内置 .NET Framework 4.x。C# 编译器和 WPF 程序集来自系统目录，无 NuGet、WebView 或额外运行框架。EXE 内置角色资料、压缩头像、图标及原生启动组件，约 2.6 MB；头像来自用户指定本地素材目录，运行时无需该目录。`gui/assets.zip` 保存 125 个角色和 514 个皮肤的目录及 192 像素头像，`gui/prepare_assets.py <Majsoul_Skins目录>` 可重新生成（开发时需要 Pillow）。

界面提供角色卡片、名称/ID 搜索、已配置筛选、独立 Mod 启停、PNG 更换与导入、路径选择和自动关闭设置。配置保存沿用 UTF-8，保留 ID、布局和注释；采用 Windows MoveFileEx 原子替换，避免 File.Replace 在部分目录中的元数据权限问题。启用相同角色/皮肤组合的新 Mod 时停用旧组合。GUI 仅写本地配置和重载命令，实际试用继续使用既有 DLL/Lua 实现。

嵌入的原生启动组件释放至 `MaJmods/launcher/engine.exe`，以 `--headless --root <工具目录>` 从后台启动。数据、DLL 和路径记忆相对于正式 EXE 所在目录。后台组件关闭标准输入，界面异步接收进度。Lua 接管确认后默认 5 秒关闭，可保留窗口或在设置中关闭倒计时。详细启动输出保存至 `MaJmods/launcher/launch.log`。

`build.ps1` 调用 `gui/build.ps1` 编译 Rust 组件及 WPF EXE。开发时可用 `gui/build.ps1 -StageOnly` 输出到 `target/gui`，避免覆盖运行文件。退出启动器后更新 EXE，退出游戏后更新 DLL。开发者可运行 `Majsoul-modLauncher.exe --self-test <报告路径>`，检查内置目录、皮肤 ID、中文配置、单个和批量 Mod 开关、角色详情、设置及搜索。

本版通过完整原生验证（13 项单元测试、957 个模块、17 个适配点及回退），图形界面自检、实际窗口预览与独立 Mod 开关验证。实际点击图形启动按钮已确认游戏启动、同目录 DLL 加载、两个 Mod 加载及 GUI/后台进程自动退出。物理 F10/F11 沿用既有实现，界面重载/恢复使用同一分发入口。旧版启动控制台的常规命令参数仍兼容。
