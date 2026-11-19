using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Markup;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Shapes;
using System.Windows.Threading;
using Microsoft.Win32;

[assembly: AssemblyTitle("Majsoul-modLauncher")]
[assembly: AssemblyProduct("Majsoul-modLauncher")]
[assembly: AssemblyVersion("0.6.9.0")]
[assembly: AssemblyFileVersion("0.6.9.0")]

namespace MJLocal {
 internal sealed class Character {
  public uint Id, DefaultSkin;
  public string Name;
  public readonly List<Skin> Skins = new List<Skin>();
  public override string ToString() { return Name + "  ·  " + Id; }
 }
 internal sealed class Skin {
  public uint Id; public string Name;
  public override string ToString() { return Name + "  ·  " + Id; }
 }
 internal sealed class Mod {
  public uint Character, Skin;
  public string Folder, Ini, Image, Name;
  public bool Enabled;
 }
 internal static class Files {
  [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)]static extern bool MoveFileEx(string source,string destination,uint flags);
  public static void Check(string path) {
   var current = new DirectoryInfo(System.IO.Path.GetDirectoryName(System.IO.Path.GetFullPath(path)));
   while (current != null) { if (current.Exists && (current.Attributes & FileAttributes.ReparsePoint) != 0) throw new IOException("此目录使用了链接，请改用普通文件夹。"); current = current.Parent; }
   if ((File.Exists(path) || Directory.Exists(path)) && (File.GetAttributes(path) & FileAttributes.ReparsePoint) != 0) throw new IOException("请使用普通文件或文件夹。");
  }
  public static string DataDirectory(string root) {
   var data=System.IO.Path.Combine(root,"MaJmods");
   Check(data);
   if(Directory.Exists(data))return data;
   if(File.Exists(data))throw new IOException("无法使用 MaJmods 目录，已有同名文件。");
   var legacy=System.IO.Path.Combine(root,"MJLocalPreview");
   if(Directory.Exists(legacy)){
    Check(legacy);
    try{Directory.Move(legacy,data);}
    catch(IOException e){
     if(Directory.Exists(legacy)||!Directory.Exists(data))
      throw new IOException("无法将 MJLocalPreview 改名为 MaJmods："+e.Message,e);
     Check(data);
    }
   }
   return data;
  }
  public static Dictionary<string,string> Ini(string path) {
   Check(path); var values = new Dictionary<string,string>(StringComparer.Ordinal);
   foreach (var source in File.ReadAllLines(path, Encoding.UTF8)) {
    var line = source.Trim().TrimStart('\uFEFF');
    if (line.Length == 0 || line.StartsWith("#") || line.StartsWith(";")) continue;
    int at = line.IndexOf('='); if (at <= 0) throw new IOException("配置行缺少等号。");
    var key = line.Substring(0,at).Trim(); if (values.ContainsKey(key)) throw new IOException("重复的配置项：" + key);
    values.Add(key,line.Substring(at+1).Trim());
   }
   return values;
  }
  public static uint Id(Dictionary<string,string> values, string name, string alias, uint fallback) {
   if (values.ContainsKey(name) && values.ContainsKey(alias)) throw new IOException("请只保留一个 ID 字段：" + name);
   string value; if (!values.TryGetValue(name,out value) && !values.TryGetValue(alias,out value)) return fallback;
   uint id; if (!uint.TryParse(value, NumberStyles.None, CultureInfo.InvariantCulture,out id)) throw new IOException("无效的 ID：" + name); return id;
  }
  public static void Atomic(string path, byte[] bytes) {
   Check(path); Directory.CreateDirectory(System.IO.Path.GetDirectoryName(path));
   var temp = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
   try { using(var file=new FileStream(temp,FileMode.CreateNew,FileAccess.Write,FileShare.None)){file.Write(bytes,0,bytes.Length);file.Flush(true);}if(!MoveFileEx(temp,path,9))throw new IOException("无法保存文件："+new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error()).Message); }
   finally { if (File.Exists(temp)) File.Delete(temp); }
  }
  public static void Text(string path,string text) { Atomic(path,new UTF8Encoding(false).GetBytes(text)); }
  public static byte[] Png(string path) {
   Check(path); var info = new FileInfo(path); if (info.Length > 64*1024*1024) throw new IOException("图片文件过大，请使用较小的 PNG。");
   var data = File.ReadAllBytes(path); byte[] signature = {137,80,78,71,13,10,26,10};
   if (data.Length < 26 || !signature.SequenceEqual(data.Take(8))) throw new IOException("请选择有效的 PNG 图片。");
   long w = 0,h = 0; for (int i=0;i<4;i++) { w = w*256+data[16+i]; h = h*256+data[20+i]; }
   if (w<1 || h<1 || w>4096 || h>8192 || w*h*4>128*1024*1024) throw new IOException("PNG 尺寸需要在 4096 × 8192 以内。");
   Assets.Bitmap(data,96); return data;
  }
  public static string Quote(string value) {
   var result = new StringBuilder("\""); int slashes=0;
   foreach (char c in value) { if (c=='\\') {slashes++;continue;} if (c=='\"') result.Append('\\',slashes*2+1); else result.Append('\\',slashes); result.Append(c); slashes=0; }
   result.Append('\\',slashes*2); return result.Append('"').ToString();
  }
 }
 internal static class Assets {
  public static readonly Dictionary<uint,Character> Characters = new Dictionary<uint,Character>();
  static readonly Dictionary<string,byte[]> Entries = new Dictionary<string,byte[]>();
  static readonly Dictionary<uint,ImageSource> Pictures = new Dictionary<uint,ImageSource>();
  public static Stream Resource(string name) { var stream=Assembly.GetExecutingAssembly().GetManifestResourceStream(name); if(stream==null)throw new IOException("启动器资源不完整："+name);return stream; }
  public static void Load() {
   if (Characters.Count > 0) return;
   using(var zip=new ZipArchive(Resource("MJ.Assets"),ZipArchiveMode.Read)) foreach(var entry in zip.Entries) using(var stream=entry.Open()) using(var data=new MemoryStream()){stream.CopyTo(data);Entries[entry.FullName]=data.ToArray();}
   foreach(var line in Encoding.UTF8.GetString(Entries["characters.tsv"]).Split('\n')) {var p=line.TrimEnd('\r').Split('\t');if(p.Length<3)continue;var c=new Character{Id=uint.Parse(p[0]),Name=p[1],DefaultSkin=uint.Parse(p[2])};Characters[c.Id]=c;}
   foreach(var line in Encoding.UTF8.GetString(Entries["skins.tsv"]).Split('\n')) {var p=line.TrimEnd('\r').Split('\t');if(p.Length<3)continue;Character c;if(Characters.TryGetValue(uint.Parse(p[0]),out c))c.Skins.Add(new Skin{Id=uint.Parse(p[1]),Name=p[2]});}
  }
  public static BitmapSource Bitmap(byte[] data,int width) {
   using(var stream=new MemoryStream(data)){var image=new BitmapImage();image.BeginInit();image.CacheOption=BitmapCacheOption.OnLoad;image.StreamSource=stream;if(width>0)image.DecodePixelWidth=width;image.EndInit();image.Freeze();return image;}
  }
  public static ImageSource Avatar(uint id) {ImageSource picture;if(Pictures.TryGetValue(id,out picture))return picture;byte[] data;if(!Entries.TryGetValue("avatars/"+id+".png",out data))return null;picture=Bitmap(data,192);Pictures[id]=picture;return picture;}
 }
 internal sealed class Shell {
  const string DllFileName="MaJmods.dll", SettingsFileName="MaJmods.ini", LegacySettingsFileName="MJLocalPreview.ini";
  public Window Window;
  readonly string root, dataRoot, modsRoot;
  readonly List<Mod> mods=new List<Mod>();
  readonly DispatcherTimer timer=new DispatcherTimer();
  string page="characters",gamePath="",fingerprint="",theme="light";
  Character selected;
  bool onlyConfigured,launching,connected,gameRunning,autoClose=true,statusIsError;
  int configErrors,closeSeconds;
  DispatcherTimer closeTimer;
  int closeAfterSeconds=5;
  [DllImport("dwmapi.dll")]static extern int DwmSetWindowAttribute(IntPtr handle,int attribute,ref int value,int size);

  static SolidColorBrush ColorBrush(string hex) {
   var brush=(SolidColorBrush)new BrushConverter().ConvertFromString(hex);
   brush.Freeze();
   return brush;
  }
  static LinearGradientBrush GradientBrush(string startHex, string endHex) {
   var start=(Color)ColorConverter.ConvertFromString(startHex);
   var end=(Color)ColorConverter.ConvertFromString(endHex);
   var brush=new LinearGradientBrush(start, end, new Point(0,0), new Point(1,1));
   brush.Freeze();
   return brush;
  }
  Brush Res(string key) { return (Brush)Window.Resources[key]; }
  T Get<T>(string name) where T:class { return Window.FindName(name) as T; }
  Style Style(string name) { return (Style)Window.FindResource(name); }

  void ApplyWindowChrome(Window target) {
   try {
    var handle=new System.Windows.Interop.WindowInteropHelper(target).Handle;
    if (handle == IntPtr.Zero) return;
    bool dark = theme == "dark";
    int darkMode = dark ? 1 : 0;
    int border = dark ? 0x3E2D28 : 0xEAE0E8;
    int caption = dark ? 0x241A17 : 0xFFFFFF;
    int text = dark ? 0xF8F4F2 : 0x2E231E;
    int corner = 2;
    DwmSetWindowAttribute(handle, 20, ref darkMode, 4);
    DwmSetWindowAttribute(handle, 34, ref border, 4);
    DwmSetWindowAttribute(handle, 35, ref caption, 4);
    DwmSetWindowAttribute(handle, 36, ref text, 4);
    DwmSetWindowAttribute(handle, 33, ref corner, 4);
   } catch (DllNotFoundException) {} catch (EntryPointNotFoundException) {}
  }

  void HookChrome(Window target) {
   target.SourceInitialized += delegate { ApplyWindowChrome(target); };
  }

  public void ApplyTheme(string mode, bool save) {
   theme = string.Equals(mode, "dark", StringComparison.OrdinalIgnoreCase) ? "dark" : "light";
   bool dark = theme == "dark";
   var r = Window.Resources;
   if (dark) {
    // 深色方案 · 玄夜鎏金 (Obsidian Slate & Luminous Rose/Gold)
    r["WindowBg"] = ColorBrush("#0E1017");
    r["ShellBg"] = ColorBrush("#12151E");
    r["SidebarBg"] = ColorBrush("#171B26");
    r["TitleBarBg"] = ColorBrush("#171B26");
    r["CardBg"] = ColorBrush("#1B202D");
    r["CardHoverBg"] = ColorBrush("#222838");
    r["CardSubtleBg"] = ColorBrush("#131722");
    r["AvatarWellBg"] = ColorBrush("#23293A");
    r["BorderBrush"] = ColorBrush("#282F42");
    r["BorderStrongBrush"] = ColorBrush("#363F57");
    r["BorderAccentBrush"] = ColorBrush("#E8749A");
    r["Ink"] = ColorBrush("#F2F4F8");
    r["InkSecondary"] = ColorBrush("#B4BDCE");
    r["Muted"] = ColorBrush("#808A9F");
    r["Subtle"] = ColorBrush("#586175");
    r["Accent"] = ColorBrush("#F2789F");
    r["AccentHover"] = ColorBrush("#FF8EB3");
    r["AccentSoft"] = ColorBrush("#2C1E2B");
    r["AccentBorder"] = ColorBrush("#5E344D");
    r["Gold"] = ColorBrush("#E6B668");
    r["GoldSoft"] = ColorBrush("#2A2319");
    r["Success"] = ColorBrush("#48C796");
    r["SuccessSoft"] = ColorBrush("#162B24");
    r["Danger"] = ColorBrush("#F26B78");
    r["DangerSoft"] = ColorBrush("#2E1B21");
    r["DangerText"] = ColorBrush("#F79AA5");
    r["BadgeBg"] = ColorBrush("#E6171B26");
    r["SwitchOffBg"] = ColorBrush("#2C3346");
    r["ScrollThumbBg"] = ColorBrush("#323A4F");
    r["ScrollThumbHoverBg"] = ColorBrush("#46506B");
    r["LaunchCardBg"] = GradientBrush("#231C2B", "#1A202E");
    r["PrimaryGradient"] = GradientBrush("#F0729B", "#D84E7B");
   } else {
    // 浅色方案 · 绯樱白玉 (Warm Alabaster & Sakura Rose)
    r["WindowBg"] = ColorBrush("#F5F3F7");
    r["ShellBg"] = ColorBrush("#FAF8FB");
    r["SidebarBg"] = ColorBrush("#FFFFFF");
    r["TitleBarBg"] = ColorBrush("#FFFFFF");
    r["CardBg"] = ColorBrush("#FFFFFF");
    r["CardHoverBg"] = ColorBrush("#FDF9FB");
    r["CardSubtleBg"] = ColorBrush("#F3EEF5");
    r["AvatarWellBg"] = ColorBrush("#ECE6F0");
    r["BorderBrush"] = ColorBrush("#E8E0EA");
    r["BorderStrongBrush"] = ColorBrush("#D9CEDC");
    r["BorderAccentBrush"] = ColorBrush("#F094B0");
    r["Ink"] = ColorBrush("#1E232E");
    r["InkSecondary"] = ColorBrush("#505664");
    r["Muted"] = ColorBrush("#888292");
    r["Subtle"] = ColorBrush("#ABA3B4");
    r["Accent"] = ColorBrush("#E65A86");
    r["AccentHover"] = ColorBrush("#D44874");
    r["AccentSoft"] = ColorBrush("#FFF0F5");
    r["AccentBorder"] = ColorBrush("#F7C2D4");
    r["Gold"] = ColorBrush("#C88832");
    r["GoldSoft"] = ColorBrush("#FDF4E7");
    r["Success"] = ColorBrush("#2D9A72");
    r["SuccessSoft"] = ColorBrush("#EAF7F1");
    r["Danger"] = ColorBrush("#D94C5A");
    r["DangerSoft"] = ColorBrush("#FDEEEF");
    r["DangerText"] = ColorBrush("#9E3345");
    r["BadgeBg"] = ColorBrush("#EBFFFFFF");
    r["SwitchOffBg"] = ColorBrush("#D8D2DC");
    r["ScrollThumbBg"] = ColorBrush("#D5CDD9");
    r["ScrollThumbHoverBg"] = ColorBrush("#B9AEBF");
    r["LaunchCardBg"] = GradientBrush("#FFF8FA", "#FFF0F5");
    r["PrimaryGradient"] = GradientBrush("#EE6892", "#DF4E7B");
   }

   ApplyWindowChrome(Window);

   var lightBtn = Get<Button>("ThemeLightButton");
   var darkBtn = Get<Button>("ThemeDarkButton");
   if (lightBtn != null && darkBtn != null) {
    lightBtn.Background = !dark ? Res("CardBg") : Brushes.Transparent;
    lightBtn.Foreground = !dark ? Res("Accent") : Res("Muted");
    darkBtn.Background = dark ? Res("CardBg") : Brushes.Transparent;
    darkBtn.Foreground = dark ? Res("Accent") : Res("Muted");
   }
   var footerTheme = Get<TextBlock>("FooterThemeText");
   if (footerTheme != null) {
    footerTheme.Text = dark ? "深色方案 · 玄夜鎏金" : "浅色方案 · 绯樱白玉";
   }
   var statusBox = Get<Border>("StatusBox");
   if (statusBox != null && statusBox.Visibility == Visibility.Visible) {
    statusBox.Background = statusIsError ? Res("DangerSoft") : Res("AccentSoft");
    statusBox.BorderBrush = statusIsError ? Res("Danger") : Res("AccentBorder");
    Get<TextBlock>("StatusText").Foreground = statusIsError ? Res("DangerText") : Res("Ink");
   }

   if (save) SavePreferences();
   RefreshGame();
   Render();
  }

  public Shell(string directory) {
   root=directory;dataRoot=Files.DataDirectory(root);modsRoot=System.IO.Path.Combine(dataRoot,"mods");
   Assets.Load();using(var stream=Assets.Resource("MJ.Window"))Window=(Window)XamlReader.Load(stream);
   using(var icon=Assets.Resource("MJ.Icon")){var image=new IconBitmapDecoder(icon,BitmapCreateOptions.None,BitmapCacheOption.OnLoad).Frames.OrderByDescending(f=>f.PixelWidth).First();image.Freeze();Window.Icon=image;}
   HookChrome(Window);Window.MinHeight=Math.Min(690,SystemParameters.WorkArea.Height);Window.MinWidth=Math.Min(1040,SystemParameters.WorkArea.Width);Window.MaxHeight=SystemParameters.WorkArea.Height;Window.Height=Math.Min(830,Window.MaxHeight);Window.Width=Math.Min(1260,SystemParameters.WorkArea.Width);
   LoadPreferences();Wire();ReadMods();ApplyTheme(theme, false);
   timer.Interval=TimeSpan.FromSeconds(2);timer.Tick+=delegate{RefreshGame();if(!launching){var old=fingerprint;ReadMods();if(old!=fingerprint){Render();}}};timer.Start();
   Window.Closing+=delegate(object sender,System.ComponentModel.CancelEventArgs e){if(launching){e.Cancel=true;Status("正在完成游戏启动，请稍候…",false);}else{timer.Stop();if(closeTimer!=null)closeTimer.Stop();}};
  }
  void Wire() {
   Get<Button>("MinButton").Click+=delegate{Window.WindowState=WindowState.Minimized;};
   Get<Button>("MaxButton").Click+=delegate{Window.WindowState=Window.WindowState==WindowState.Maximized?WindowState.Normal:WindowState.Maximized;};
   Get<Button>("CloseButton").Click+=delegate{Window.Close();};
   Get<Border>("TitleBar").MouseLeftButtonDown+=delegate(object sender,MouseButtonEventArgs e){if(e.ClickCount==2)Window.WindowState=Window.WindowState==WindowState.Maximized?WindowState.Normal:WindowState.Maximized;else if(e.ButtonState==MouseButtonState.Pressed)Window.DragMove();};
   Get<Button>("ThemeLightButton").Click+=delegate{if(theme!="light")ApplyTheme("light",true);};
   Get<Button>("ThemeDarkButton").Click+=delegate{if(theme!="dark")ApplyTheme("dark",true);};
   Get<Button>("NavCharacters").Click+=delegate{Navigate("characters");};Get<Button>("NavSettings").Click+=delegate{Navigate("settings");};
   var searchBox=Get<TextBox>("SearchBox");
   var clearBtn=Get<Button>("SearchClearButton");
   searchBox.TextChanged+=delegate{
    bool empty=searchBox.Text.Length==0;
    Get<TextBlock>("SearchHint").Visibility=empty?Visibility.Visible:Visibility.Collapsed;
    if(clearBtn!=null)clearBtn.Visibility=empty?Visibility.Collapsed:Visibility.Visible;
    if(page=="characters")Render();
   };
   if(clearBtn!=null)clearBtn.Click+=delegate{searchBox.Text="";searchBox.Focus();};
   Get<Button>("ShowAllButton").Click+=delegate{onlyConfigured=false;Render();};Get<Button>("ShowConfiguredButton").Click+=delegate{onlyConfigured=true;Render();};
   Get<Button>("EnableAllModsButton").Click+=delegate{SetAllModsEnabled(true);};
   Get<Button>("DisableAllModsButton").Click+=delegate{SetAllModsEnabled(false);};
   Get<Button>("LaunchButton").Click+=delegate{StartGame();};
   Get<Button>("ReloadButton").Click+=delegate{SendCommand("reload");};Get<Button>("RestoreButton").Click+=delegate{SendCommand("close");};
   Get<Button>("KeepOpenButton").Click+=delegate{if(closeTimer!=null)closeTimer.Stop();Get<Button>("KeepOpenButton").Visibility=Visibility.Collapsed;Status("游戏已启动，可以继续管理本地 Mod。",false);};
  }
  void Navigate(string name) {page=name;selected=null;if(!launching&&closeTimer==null)Get<Border>("StatusBox").Visibility=Visibility.Collapsed;Get<TextBox>("SearchBox").Text="";Get<ScrollViewer>("PageScroll").ScrollToTop();Render();}
  void LoadPreferences() {
   try{
    var path=System.IO.Path.Combine(root,SettingsFileName);
    var legacy=System.IO.Path.Combine(root,LegacySettingsFileName);
    if(!File.Exists(path)&&File.Exists(legacy)){
     Files.Check(path);Files.Check(legacy);
     try{File.Move(legacy,path);}
     catch(IOException){if(!File.Exists(path))path=legacy;}
     catch(UnauthorizedAccessException){if(!File.Exists(path))path=legacy;}
    }
    if(File.Exists(path)){
     Files.Check(path);
     var text=File.ReadAllText(path,Encoding.UTF8).TrimStart('\uFEFF').TrimEnd('\r','\n');
     gamePath=text.StartsWith("game=",StringComparison.Ordinal)?text.Substring(5):text;
    }
   }
   catch{gamePath="";}
   try{
    var path=System.IO.Path.Combine(dataRoot,"launcher","ui.ini");
    if(File.Exists(path)){
     var v=Files.Ini(path);
     string s;
     if(v.TryGetValue("close_after_launch",out s))autoClose=s!="0";
     if(v.TryGetValue("theme",out s)&&(string.Equals(s,"dark",StringComparison.OrdinalIgnoreCase)||string.Equals(s,"light",StringComparison.OrdinalIgnoreCase)))theme=s.ToLowerInvariant();
    }
   }catch{}
  }
  void SavePreferences(){Files.Text(System.IO.Path.Combine(dataRoot,"launcher","ui.ini"),"close_after_launch="+(autoClose?"1":"0")+"\ntheme="+theme+"\n");}
  void ReadMods() {
   var result=new List<Mod>();var stamp=new StringBuilder();configErrors=0;
   try{if(Directory.Exists(modsRoot)){Files.Check(modsRoot);foreach(var role in Directory.GetDirectories(modsRoot)){Files.Check(role);foreach(var folder in Directory.GetDirectories(role)){
    try{Files.Check(folder);var ini=System.IO.Path.Combine(folder,"mod.ini");if(!File.Exists(ini))continue;stamp.Append(ini).Append(File.GetLastWriteTimeUtc(ini).Ticks);var values=Files.Ini(ini);var id=Files.Id(values,"character_id","character",0);if(id==0)throw new IOException("缺少角色 ID");var skin=Files.Id(values,"skin_id","skin",0);string value;
     var enabled=!values.TryGetValue("enabled",out value)||value=="1";if(value!=null&&value!="0"&&value!="1")throw new IOException("enabled 需为 0 或 1");
     var image=values.TryGetValue("image",out value)?value:"custom.png";if(System.IO.Path.IsPathRooted(image))throw new IOException("图片需放在这个 Mod 目录内");
     image=System.IO.Path.GetFullPath(System.IO.Path.Combine(folder,image));if(!image.StartsWith(folder.TrimEnd('\\')+"\\",StringComparison.OrdinalIgnoreCase))throw new IOException("图片不在 Mod 目录内");Files.Check(image);
     var mod=new Mod{Character=id,Skin=skin,Enabled=enabled,Folder=folder,Ini=ini,Image=image,Name=System.IO.Path.GetFileName(folder)};result.Add(mod);if(File.Exists(image))stamp.Append(File.GetLastWriteTimeUtc(image).Ticks);
     if(!Assets.Characters.ContainsKey(id))Assets.Characters[id]=new Character{Id=id,Name=System.IO.Path.GetFileName(role),DefaultSkin=skin};
    }catch{configErrors++;}
   }}}}catch{configErrors++;}
   mods.Clear();mods.AddRange(result);fingerprint=stamp.ToString()+"|"+configErrors;
  }
  void RefreshGame() {
   gameRunning=false;connected=false;
   var runtimeDll=System.IO.Path.Combine(root,DllFileName);
   foreach(var process in Process.GetProcessesByName("Jantama_MahjongSoul"))using(process){gameRunning=true;try{foreach(ProcessModule module in process.Modules){if(string.Equals(module.FileName,runtimeDll,StringComparison.OrdinalIgnoreCase)){connected=true;break;}}}catch{}}
   Get<Button>("LaunchButton").IsEnabled=!launching&&!gameRunning;
   Get<Button>("LaunchButton").Content=launching?"正在启动…":gameRunning?"游戏运行中":"↗  启动游戏";
   Get<Button>("ReloadButton").IsEnabled=connected&&!launching;Get<Button>("RestoreButton").IsEnabled=connected&&!launching;
   Get<Border>("BulkModControls").IsEnabled=!launching;
   Get<TextBlock>("GameState").Text=launching?"正在准备试用…":connected?"已连接本地试用":gameRunning?"请从本工具重启":"准备就绪";
   var stateColor=connected?Res("Success"):launching?Res("Accent"):gameRunning?Res("Gold"):Res("Muted");
   Get<TextBlock>("GameState").Foreground=stateColor;
   var dot=Get<Ellipse>("GameStateDot");
   if(dot!=null)dot.Fill=stateColor;
   Get<TextBlock>("GamePathShort").Text=File.Exists(gamePath)?System.IO.Path.GetFileName(gamePath):"首次启动时选择游戏";
   Get<TextBlock>("GamePathShort").ToolTip=File.Exists(gamePath)?gamePath:"点击“启动游戏”或前往“启动与外观设置”选择雀魂 EXE";
  }
  TextBlock Text(string value,double size,Brush color=null,FontWeight? weight=null){
   return new TextBlock{Text=value,FontSize=size,Foreground=color??Res("Ink"),FontWeight=weight??FontWeights.Normal,TextWrapping=TextWrapping.Wrap};
  }
  Button Button(string caption,Action action,bool primary=false){
   var button=new Button{Content=caption,Style=Style(primary?"Primary":"SoftButton")};
   button.Click+=delegate{try{action();}catch(Exception e){Status(e.Message,true);}};
   return button;
  }
  Border Pill(string text, Brush fg, Brush bg, Brush border=null) {
   return new Border {
    Background = bg,
    BorderBrush = border ?? Brushes.Transparent,
    BorderThickness = new Thickness(border != null ? 1 : 0),
    CornerRadius = new CornerRadius(7),
    Padding = new Thickness(8, 2.5, 8, 2.5),
    Margin = new Thickness(0, 0, 6, 0),
    VerticalAlignment = VerticalAlignment.Center,
    Child = new TextBlock { Text = text, FontSize = 10.5, FontWeight = FontWeights.SemiBold, Foreground = fg }
   };
  }
  void Status(string text,bool error){
   statusIsError=error;
   var box=Get<Border>("StatusBox");
   box.Visibility=Visibility.Visible;
   box.Background=error?Res("DangerSoft"):Res("AccentSoft");
   box.BorderBrush=error?Res("Danger"):Res("AccentBorder");
   var label=Get<TextBlock>("StatusText");
   label.Text=text;
   label.Foreground=error?Res("DangerText"):Res("Ink");
  }
  void Render() {
   if(Window==null)return;
   var navEntries=new[]{
    new{Name="NavCharacters",Bar="NavCharactersBar",Active=page=="characters"||page=="role"},
    new{Name="NavSettings",Bar="NavSettingsBar",Active=page=="settings"}
   };
   foreach(var entry in navEntries){
    var button=Get<Button>(entry.Name);
    button.Background=entry.Active?Res("AccentSoft"):Brushes.Transparent;
    button.BorderBrush=entry.Active?Res("AccentBorder"):Brushes.Transparent;
    button.Foreground=entry.Active?Res("Accent"):Res("InkSecondary");
    var bar=Get<Border>(entry.Bar);
    if(bar!=null)bar.Background=entry.Active?Res("Accent"):Brushes.Transparent;
   }
   var charBadge=Get<TextBlock>("NavCharactersBadge");
   if(charBadge!=null)charBadge.Text=Assets.Characters.Count.ToString();
   int enabledTotal=mods.Count(mod=>mod.Enabled);
   int configuredRoles=Assets.Characters.Values.Count(c=>mods.Any(m=>m.Character==c.Id));
   Get<TextBlock>("Summary").Text=Assets.Characters.Count+" 位雀士（"+configuredRoles+" 位已配置）    ·    "+mods.Count+" 个本地 Mod    ·    "+enabledTotal+" 个已启用";
   Get<Border>("FilterBar").Visibility=page=="characters"?Visibility.Visible:Visibility.Collapsed;
   Get<Button>("ShowAllButton").Background=onlyConfigured?Brushes.Transparent:Res("CardBg");
   Get<Button>("ShowAllButton").Foreground=onlyConfigured?Res("Muted"):Res("Accent");
   Get<Button>("ShowConfiguredButton").Background=onlyConfigured?Res("CardBg"):Brushes.Transparent;
   Get<Button>("ShowConfiguredButton").Foreground=onlyConfigured?Res("Accent"):Res("Muted");
   Get<StackPanel>("PageBody").Children.Clear();
   if(page=="role"&&selected!=null)RenderRole();else if(page=="settings")RenderSettings();else RenderCharacters();
   if(configErrors>0)Status("有 "+configErrors+" 个 Mod 配置无法读取，请检查对应的 mod.ini。",true);
  }
  void RenderCharacters() {
   Get<TextBlock>("PageTitle").Text="角色一览";Get<TextBlock>("PageSubtitle").Text="挑选一位雀士，为喜欢的皮肤换上新画面";
   bool hasMods=mods.Count>0;
   bool allEnabled=hasMods&&PreferredMods().SetEquals(mods.Where(mod=>mod.Enabled));
   bool allDisabled=hasMods&&!mods.Any(mod=>mod.Enabled);
   var enableAll=Get<Button>("EnableAllModsButton");
   var disableAll=Get<Button>("DisableAllModsButton");
   enableAll.IsEnabled=disableAll.IsEnabled=hasMods;
   enableAll.Background=allEnabled?Res("CardBg"):Brushes.Transparent;
   enableAll.Foreground=allEnabled?Res("Accent"):Res("InkSecondary");
   disableAll.Background=allDisabled?Res("CardBg"):Brushes.Transparent;
   disableAll.Foreground=allDisabled?Res("Accent"):Res("InkSecondary");
   var panel=new WrapPanel{Orientation=Orientation.Horizontal};Get<StackPanel>("PageBody").Children.Add(panel);
   var add=Button("",delegate{Import(null);});add.Width=168;add.Height=214;add.Margin=new Thickness(0,0,16,18);add.Padding=new Thickness(0);add.Background=Res("CardBg");add.BorderBrush=Res("BorderStrongBrush");
   var plus=new StackPanel{VerticalAlignment=VerticalAlignment.Center,HorizontalAlignment=HorizontalAlignment.Center};
   var iconCircle=new Border{Width=48,Height=48,CornerRadius=new CornerRadius(24),Background=Res("AccentSoft"),BorderBrush=Res("AccentBorder"),BorderThickness=new Thickness(1),HorizontalAlignment=HorizontalAlignment.Center,Child=new TextBlock{Text="+",FontSize=26,FontWeight=FontWeights.SemiBold,Foreground=Res("Accent"),HorizontalAlignment=HorizontalAlignment.Center,VerticalAlignment=VerticalAlignment.Center,Margin=new Thickness(0,-2,0,0)}};
   plus.Children.Add(iconCircle);
   plus.Children.Add(new TextBlock{Text="添加自定义图片",FontSize=13,FontWeight=FontWeights.SemiBold,Foreground=Res("Ink"),Margin=new Thickness(0,12,0,0),HorizontalAlignment=HorizontalAlignment.Center});
   plus.Children.Add(new TextBlock{Text="导入 PNG 替换皮肤",FontSize=10.5,Foreground=Res("Muted"),Margin=new Thickness(0,4,0,0),HorizontalAlignment=HorizontalAlignment.Center});
   add.Content=plus;panel.Children.Add(add);
   var query=Get<TextBox>("SearchBox").Text.Trim();var comparer=StringComparer.Create(CultureInfo.GetCultureInfo("zh-CN"),true);
   var roles=Assets.Characters.Values.Where(c=>(!onlyConfigured||mods.Any(m=>m.Character==c.Id))&&(query.Length==0||c.Name.IndexOf(query,StringComparison.OrdinalIgnoreCase)>=0||c.Id.ToString().Contains(query))).OrderByDescending(c=>mods.Any(m=>m.Character==c.Id)).ThenBy(c=>c.Name,comparer).ToList();
   var countLabel=Get<TextBlock>("FilterCountText");
   if(countLabel!=null)countLabel.Text="显示 "+roles.Count+" / "+Assets.Characters.Count+" 位";
   foreach(var character in roles)panel.Children.Add(CharacterCard(character));
   if(roles.Count==0){var empty=Text("没有找到匹配的角色。试试角色名称或 6 位角色 ID。",13,Res("Muted"));empty.Margin=new Thickness(4,36,0,0);Get<StackPanel>("PageBody").Children.Add(empty);}
  }
  Button CharacterCard(Character character) {
   var mine=mods.Where(m=>m.Character==character.Id).ToList();
   int enabledCount=mine.Count(m=>m.Enabled);
   var card=Button("",delegate{selected=character;page="role";Get<ScrollViewer>("PageScroll").ScrollToTop();Render();});
   card.Width=168;card.Height=214;card.Padding=new Thickness(0);card.Margin=new Thickness(0,0,16,18);
   card.Background=Res("CardBg");
   card.BorderBrush=enabledCount>0?Res("AccentBorder"):Res("BorderBrush");
   card.ToolTip=character.Name+" · ID "+character.Id+"（"+character.Skins.Count+" 款皮肤）";
   var grid=new Grid();
   grid.RowDefinitions.Add(new RowDefinition{Height=new GridLength(156)});
   grid.RowDefinitions.Add(new RowDefinition{Height=new GridLength(56)});
   var photo=new Grid{ClipToBounds=true};
   photo.Children.Add(new Border{CornerRadius=new CornerRadius(10,10,0,0),Background=Res("AvatarWellBg")});
   var avatar=Assets.Avatar(character.Id);
   if(avatar!=null){
    photo.Children.Add(new Rectangle{Width=166,Height=174,RadiusX=10,RadiusY=10,VerticalAlignment=VerticalAlignment.Top,Fill=new ImageBrush(avatar){Stretch=Stretch.UniformToFill}});
   }else{
    photo.Children.Add(new TextBlock{Text=character.Name.Substring(0,1),FontSize=52,FontWeight=FontWeights.Bold,Foreground=Res("Accent"),HorizontalAlignment=HorizontalAlignment.Center,VerticalAlignment=VerticalAlignment.Center});
   }
   photo.Children.Add(new Border{Height=1,Background=Res("BorderBrush"),VerticalAlignment=VerticalAlignment.Bottom});
   if(enabledCount>0){
    var activeTag=new Border{Background=Res("Accent"),CornerRadius=new CornerRadius(7),Padding=new Thickness(7,2,7,2),HorizontalAlignment=HorizontalAlignment.Left,VerticalAlignment=VerticalAlignment.Top,Margin=new Thickness(8,8,0,0)};
    activeTag.Child=new TextBlock{Text="● 已配置",FontSize=9.5,FontWeight=FontWeights.SemiBold,Foreground=Brushes.White};
    photo.Children.Add(activeTag);
   }
   var badge=new Border{Background=Res("BadgeBg"),BorderBrush=Res("BorderBrush"),BorderThickness=new Thickness(1),CornerRadius=new CornerRadius(9),Padding=new Thickness(8,2.5,8,2.5),HorizontalAlignment=HorizontalAlignment.Right,VerticalAlignment=VerticalAlignment.Bottom,Margin=new Thickness(0,0,8,8),ToolTip="已启用 / 已安装 Mod"};
   var counts=new TextBlock{FontSize=10.5,FontWeight=FontWeights.SemiBold,Text="◈  "+enabledCount+"/"+mine.Count,Foreground=mine.Count>0?Res("Accent"):Res("Muted")};
   badge.Child=counts;photo.Children.Add(badge);grid.Children.Add(photo);

   var footer=new StackPanel{VerticalAlignment=VerticalAlignment.Center,Margin=new Thickness(10,0,10,0)};
   var name=new TextBlock{Text=character.Name,FontSize=13.5,FontWeight=FontWeights.SemiBold,Foreground=Res("Ink"),TextTrimming=TextTrimming.CharacterEllipsis,HorizontalAlignment=HorizontalAlignment.Center,MaxWidth=148};
   var meta=new TextBlock{Text="ID "+character.Id+(character.Skins.Count>0?" · "+character.Skins.Count+" 皮肤":""),FontSize=10,Foreground=Res("Muted"),HorizontalAlignment=HorizontalAlignment.Center,Margin=new Thickness(0,3,0,0)};
   footer.Children.Add(name);footer.Children.Add(meta);
   Grid.SetRow(footer,1);grid.Children.Add(footer);card.Content=grid;return card;
  }
  Border Card(UIElement child, bool highlight=false) {
   return new Border{
    Background=Res("CardBg"),
    BorderBrush=highlight?Res("AccentBorder"):Res("BorderBrush"),
    BorderThickness=new Thickness(1),
    CornerRadius=new CornerRadius(15),
    Padding=new Thickness(20,16,20,16),
    Margin=new Thickness(0,0,0,12),
    Child=child
   };
  }
  void RenderRole() {
   var c=selected;
   var mine=mods.Where(m=>m.Character==c.Id).OrderBy(m=>m.Skin).ThenBy(m=>m.Name).ToList();
   Get<TextBlock>("PageTitle").Text=c.Name;
   Get<TextBlock>("PageSubtitle").Text="角色 ID "+c.Id+"  ·  内置 "+c.Skins.Count+" 款皮肤资料  ·  已配置 "+mine.Count+" 个 Mod";
   var actions=new Grid{Margin=new Thickness(0,0,0,18)};
   actions.ColumnDefinitions.Add(new ColumnDefinition{Width= new GridLength(1,GridUnitType.Star)});
   actions.ColumnDefinitions.Add(new ColumnDefinition{Width=GridLength.Auto});
   var leftBtns=new StackPanel{Orientation=Orientation.Horizontal};
   leftBtns.Children.Add(Button("←  返回角色一览",delegate{Navigate("characters");}));
   var add=Button("+  添加自定义图片",delegate{Import(c);},true);add.Margin=new Thickness(10,0,0,0);leftBtns.Children.Add(add);
   actions.Children.Add(leftBtns);
   var folderBtn=Button("打开角色目录",delegate{OpenFolder(System.IO.Path.Combine(modsRoot,c.Name));});
   Grid.SetColumn(folderBtn,1);actions.Children.Add(folderBtn);
   Get<StackPanel>("PageBody").Children.Add(actions);

   if(mine.Count==0){
    var empty=new StackPanel{Margin=new Thickness(8,18,8,18)};
    empty.Children.Add(Text("还没有为「"+c.Name+"」添加自定义立绘",17,Res("Ink"),FontWeights.SemiBold));
    var note=Text("选择一款皮肤并导入 PNG 图片，立绘尺寸、缩放与裁切关系会自动跟随游戏原画对齐。",12.5,Res("Muted"));
    note.Margin=new Thickness(0,9,0,16);empty.Children.Add(note);
    var quickAdd=Button("+  立即为 "+c.Name+" 添加图片",delegate{Import(c);},true);
    quickAdd.HorizontalAlignment=HorizontalAlignment.Left;quickAdd.Padding=new Thickness(18,9,18,9);
    empty.Children.Add(quickAdd);
    Get<StackPanel>("PageBody").Children.Add(Card(empty));
   }
   foreach(var mod in mine)Get<StackPanel>("PageBody").Children.Add(ModCard(mod));
  }
  Border ModCard(Mod mod) {
   var c=Assets.Characters[mod.Character];
   var skin=c.Skins.FirstOrDefault(s=>s.Id==mod.Skin);
   var skinName=mod.Skin==0?"角色通用（全部皮肤）":skin!=null?(mod.Skin==c.DefaultSkin?"默认皮肤 · "+skin.Name:skin.Name):"指定皮肤";
   var row=new Grid();
   row.ColumnDefinitions.Add(new ColumnDefinition{Width=new GridLength(128)});
   row.ColumnDefinitions.Add(new ColumnDefinition{Width=new GridLength(1,GridUnitType.Star)});
   row.ColumnDefinitions.Add(new ColumnDefinition{Width=GridLength.Auto});

   var preview=new Border{Width=110,Height=134,CornerRadius=new CornerRadius(12),Background=Res("CardSubtleBg"),BorderBrush=Res("BorderBrush"),BorderThickness=new Thickness(1),ClipToBounds=true,HorizontalAlignment=HorizontalAlignment.Left};
   try{
    Files.Check(mod.Image);
    preview.Child=new Image{Source=Assets.Bitmap(File.ReadAllBytes(mod.Image),240),Stretch=Stretch.Uniform,Margin=new Thickness(4)};
   }catch{
    preview.Child=new TextBlock{Text="图片未找到",FontSize=11,Foreground=Res("Muted"),VerticalAlignment=VerticalAlignment.Center,HorizontalAlignment=HorizontalAlignment.Center};
   }
   row.Children.Add(preview);

   var info=new StackPanel{VerticalAlignment=VerticalAlignment.Center};Grid.SetColumn(info,1);row.Children.Add(info);
   var titleRow=new StackPanel{Orientation=Orientation.Horizontal,VerticalAlignment=VerticalAlignment.Center};
   titleRow.Children.Add(Text(mod.Name,16.5,Res("Ink"),FontWeights.Bold));
   var statusPill=Pill(mod.Enabled?"● 已启用":"○ 已停用",mod.Enabled?Res("Accent"):Res("Muted"),mod.Enabled?Res("AccentSoft"):Res("CardSubtleBg"),mod.Enabled?Res("AccentBorder"):Res("BorderBrush"));
   statusPill.Margin=new Thickness(10,0,0,0);
   titleRow.Children.Add(statusPill);
   info.Children.Add(titleRow);

   var sub=Text(c.Name+"  /  "+skinName,12.5,Res("InkSecondary"),FontWeights.SemiBold);
   sub.Margin=new Thickness(0,8,0,0);info.Children.Add(sub);

   var pills=new StackPanel{Orientation=Orientation.Horizontal,Margin=new Thickness(0,8,0,0)};
   pills.Children.Add(Pill("皮肤 ID  "+mod.Skin,Res("Muted"),Res("CardSubtleBg"),Res("BorderBrush")));
   pills.Children.Add(Pill("角色 ID  "+mod.Character,Res("Muted"),Res("CardSubtleBg"),Res("BorderBrush")));
   pills.Children.Add(Pill(System.IO.Path.GetFileName(mod.Image),Res("Muted"),Res("CardSubtleBg"),Res("BorderBrush")));
   info.Children.Add(pills);

   var commands=new StackPanel{Orientation=Orientation.Horizontal,Margin=new Thickness(0,14,0,0)};
   var replace=Button("更换图片",delegate{ReplaceImage(mod);});replace.FontSize=11.5;replace.Padding=new Thickness(13,6,13,6);commands.Children.Add(replace);
   foreach(var kind in new[]{"smallhead","bighead"}) {
     string avatarKind=kind;
     var avatar=Button(kind=="smallhead"?"更换小头像":"更换大头像",delegate{ReplaceAvatar(mod,avatarKind);});
     avatar.FontSize=11.5;avatar.Padding=new Thickness(13,6,13,6);avatar.Margin=new Thickness(8,0,0,0);commands.Children.Add(avatar);
    }
    var folder=Button("打开文件夹",delegate{OpenFolder(mod.Folder);});folder.FontSize=11.5;folder.Padding=new Thickness(13,6,13,6);folder.Margin=new Thickness(8,0,0,0);commands.Children.Add(folder);
   info.Children.Add(commands);

   var state=new StackPanel{VerticalAlignment=VerticalAlignment.Center,Margin=new Thickness(16,0,0,0)};Grid.SetColumn(state,2);
   var toggle=new CheckBox{IsChecked=mod.Enabled,Style=Style("Switch"),HorizontalAlignment=HorizontalAlignment.Center,ToolTip="同一角色与皮肤只启用一个 Mod"};
   toggle.Click+=delegate{try{bool enabled=toggle.IsChecked==true;if(enabled)foreach(var old in mods.Where(m=>m.Enabled&&m.Character==mod.Character&&m.Skin==mod.Skin&&m.Ini!=mod.Ini))SetEnabled(old,false);SetEnabled(mod,enabled);ReadMods();Render();Saved();}catch(Exception e){toggle.IsChecked=mod.Enabled;Status(e.Message,true);}};
   state.Children.Add(toggle);
   state.Children.Add(new TextBlock{Text=mod.Enabled?"已启用":"已停用",FontSize=10.5,FontWeight=FontWeights.SemiBold,Foreground=mod.Enabled?Res("Accent"):Res("Muted"),HorizontalAlignment=HorizontalAlignment.Center,Margin=new Thickness(0,8,0,0)});
   row.Children.Add(state);
   return Card(row, mod.Enabled);
  }
  void SetEnabled(Mod mod,bool enabled) {
   Files.Ini(mod.Ini);var text=File.ReadAllText(mod.Ini,Encoding.UTF8).TrimStart('\uFEFF');var regex=new Regex(@"(?m)^\s*enabled\s*=[^\r\n]*");var line="enabled = "+(enabled?"1":"0");text=regex.IsMatch(text)?regex.Replace(text,line,1):line+"\n"+text;Files.Text(mod.Ini,text);mod.Enabled=enabled;
  }
  HashSet<Mod> PreferredMods() {
   return new HashSet<Mod>(mods.GroupBy(mod=>new{mod.Character,mod.Skin})
    .Select(group=>group.OrderByDescending(mod=>mod.Enabled).ThenBy(mod=>mod.Ini,StringComparer.OrdinalIgnoreCase).First()));
  }
  void SetAllModsEnabled(bool enabled) {
   ReadMods();
   var targets=enabled?PreferredMods():new HashSet<Mod>();
   var changes=mods.Where(mod=>mod.Enabled!=targets.Contains(mod)).OrderBy(mod=>targets.Contains(mod));
   int updated=0, failed=0;
   string firstError=null;
   foreach(var mod in changes){
    try{SetEnabled(mod,targets.Contains(mod));updated++;}
    catch(Exception e){failed++;if(firstError==null)firstError=mod.Name+"："+e.Message;}
   }
   ReadMods();
   if(updated>0||failed==0)Saved();
   Render();
   if(failed>0)Status("已切换 "+updated+" 个 Mod，"+failed+" 个切换失败。"+firstError,true);
  }
  void ReplaceImage(Mod mod) {var choose=new OpenFileDialog{Title="选择替换图片",Filter="PNG 图片 (*.png)|*.png",CheckFileExists=true};if(choose.ShowDialog(Window)!=true)return;var bytes=Files.Png(choose.FileName);Files.Atomic(mod.Image,bytes);ReadMods();Render();Saved();}
  void ReplaceAvatar(Mod mod,string kind) {
   var choose=new OpenFileDialog{Title=kind=="smallhead"?"选择游戏内小头像":"选择游戏内大头像",Filter="PNG 图片 (*.png)|*.png",CheckFileExists=true};
   if(choose.ShowDialog(Window)!=true)return;
   try{Files.Atomic(System.IO.Path.Combine(mod.Folder,kind+".png"),Files.Png(choose.FileName));ReadMods();Render();Saved();}
   catch(Exception e){Status(e.Message,true);}
  }
  void Saved(){if(connected)SendCommand("reload");else Status("已保存，启动游戏后生效。",false);}
  void SendCommand(string command) {
   RefreshGame();if(!connected){Status("请先使用本工具启动游戏，再重载或恢复图片。",false);return;}
   try{Files.Text(System.IO.Path.Combine(dataRoot,"command.txt"),command);Status(command=="close"?"已请求恢复全部原图。配置会保留，按 F10 可再次启用。":"已保存并请求重载。游戏中的 F10 / F11 仍可使用。",false);}catch(Exception e){Status("操作未完成："+e.Message,true);}
  }
  void OpenFolder(string folder){Files.Check(folder);Directory.CreateDirectory(folder);Process.Start(new ProcessStartInfo{FileName=folder,UseShellExecute=true});}

  Border ThemeOptionCard(string modeKey, string title, string subtitle, string bgHex, string cardHex, string accentHex, string textHex) {
   bool active = string.Equals(theme, modeKey, StringComparison.OrdinalIgnoreCase);
   var card = new Border {
    Background = active ? Res("AccentSoft") : Res("CardSubtleBg"),
    BorderBrush = active ? Res("Accent") : Res("BorderBrush"),
    BorderThickness = new Thickness(active ? 1.5 : 1),
    CornerRadius = new CornerRadius(12),
    Padding = new Thickness(15, 13, 15, 13),
    Cursor = Cursors.Hand
   };
   var layout = new Grid();
   layout.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(56) });
   layout.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
   layout.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

   // Mini theme preview swatch
   var swatch = new Border {
    Width = 46, Height = 38,
    CornerRadius = new CornerRadius(8),
    Background = ColorBrush(bgHex),
    BorderBrush = ColorBrush(accentHex),
    BorderThickness = new Thickness(1),
    HorizontalAlignment = HorizontalAlignment.Left,
    VerticalAlignment = VerticalAlignment.Center,
    Padding = new Thickness(5)
   };
   var swatchInner = new Grid();
   swatchInner.RowDefinitions.Add(new RowDefinition { Height = new GridLength(10) });
   swatchInner.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
   var dotRow = new StackPanel { Orientation = Orientation.Horizontal };
   dotRow.Children.Add(new Ellipse { Width = 6, Height = 6, Fill = ColorBrush(accentHex), Margin = new Thickness(0, 0, 4, 0) });
   dotRow.Children.Add(new Rectangle { Width = 18, Height = 4, RadiusX = 2, RadiusY = 2, Fill = ColorBrush(textHex), VerticalAlignment = VerticalAlignment.Center });
   swatchInner.Children.Add(dotRow);
   var miniCard = new Border { Background = ColorBrush(cardHex), CornerRadius = new CornerRadius(4), Margin = new Thickness(0, 3, 0, 0) };
   Grid.SetRow(miniCard, 1);
   swatchInner.Children.Add(miniCard);
   swatch.Child = swatchInner;
   layout.Children.Add(swatch);

   var texts = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
   Grid.SetColumn(texts, 1);
   texts.Children.Add(Text(title, 13.5, active ? Res("Accent") : Res("Ink"), FontWeights.SemiBold));
   var sub = Text(subtitle, 11, Res("Muted"));
   sub.Margin = new Thickness(0, 3, 0, 0);
   texts.Children.Add(sub);
   layout.Children.Add(texts);

   var statePill = Pill(active ? "✓ 当前方案" : "点击启用", active ? Res("Accent") : Res("Muted"), active ? Res("CardBg") : Brushes.Transparent, active ? Res("AccentBorder") : Res("BorderBrush"));
   Grid.SetColumn(statePill, 2);
   layout.Children.Add(statePill);

   card.Child = layout;
   card.MouseLeftButtonDown += delegate {
    if (!string.Equals(theme, modeKey, StringComparison.OrdinalIgnoreCase)) ApplyTheme(modeKey, true);
   };
   return card;
  }

  void RenderSettings() {
   Get<TextBlock>("PageTitle").Text="启动与外观设置";Get<TextBlock>("PageSubtitle").Text="配置客户端位置、切换深色/浅色视觉主题与启动行为";

   // Card 1: Game Executable Path
   var game=new StackPanel();
   var gameHeader=new StackPanel{Orientation=Orientation.Horizontal,VerticalAlignment=VerticalAlignment.Center};
   gameHeader.Children.Add(Text("游戏启动程序",16,Res("Ink"),FontWeights.Bold));
   var validPath=File.Exists(gamePath);
   var pathBadge=Pill(validPath?"● 已记住路径":"○ 待选择",validPath?Res("Success"):Res("Muted"),validPath?Res("SuccessSoft"):Res("CardSubtleBg"));
   pathBadge.Margin=new Thickness(10,0,0,0);
   gameHeader.Children.Add(pathBadge);
   game.Children.Add(gameHeader);
   var explanation=Text("选择雀魂客户端的启动 EXE（Jantama_MahjongSoul.exe），工具会自动将路径保存在 "+SettingsFileName+"。",12,Res("Muted"));
   explanation.Margin=new Thickness(0,7,0,15);game.Children.Add(explanation);
   var pathRow=new Grid();
   pathRow.ColumnDefinitions.Add(new ColumnDefinition{Width=new GridLength(1,GridUnitType.Star)});
   pathRow.ColumnDefinitions.Add(new ColumnDefinition{Width=GridLength.Auto});
   var pathBox=new TextBox{Text=validPath?gamePath:"尚未选择游戏程序（点击右侧按钮或启动游戏时选择）",IsReadOnly=true,Background=Res("CardSubtleBg"),BorderBrush=Res("BorderBrush"),BorderThickness=new Thickness(1),Foreground=validPath?Res("Ink"):Res("Muted"),FontSize=11.5,Margin=new Thickness(0,0,10,0),MinHeight=38,VerticalContentAlignment=VerticalAlignment.Center};
   pathRow.Children.Add(pathBox);
   var pick=Button("选择程序…",delegate{if(ChooseGame()){RefreshGame();Render();}});
   Grid.SetColumn(pick,1);pathRow.Children.Add(pick);game.Children.Add(pathRow);
   Get<StackPanel>("PageBody").Children.Add(Card(game));

   // Card 2: Appearance Theme (Light / Dark) + Launch Behavior
   var prefs=new StackPanel();
   prefs.Children.Add(Text("界面主题与启动偏好",16,Res("Ink"),FontWeights.Bold));
   var themeNote=Text("提供浅色与深色两种视觉方案，随时可在顶部标题栏或此处一键切换。",12,Res("Muted"));
   themeNote.Margin=new Thickness(0,7,0,14);prefs.Children.Add(themeNote);

   var themeGrid=new Grid();
   themeGrid.ColumnDefinitions.Add(new ColumnDefinition{Width=new GridLength(1,GridUnitType.Star)});
   themeGrid.ColumnDefinitions.Add(new ColumnDefinition{Width=new GridLength(14)});
   themeGrid.ColumnDefinitions.Add(new ColumnDefinition{Width=new GridLength(1,GridUnitType.Star)});
   var lightCard=ThemeOptionCard("light","☀  浅色方案 · 绯樱白玉","温润米白与樱粉配色，明亮清爽","#FAF8FB","#FFFFFF","#E65A86","#1E232E");
   var darkCard=ThemeOptionCard("dark","☾  深色方案 · 玄夜鎏金","暗夜深灰与绯金点缀，柔和护眼","#12151E","#1B202D","#F2789F","#F2F4F8");
   Grid.SetColumn(darkCard,2);
   themeGrid.Children.Add(lightCard);themeGrid.Children.Add(darkCard);
   prefs.Children.Add(themeGrid);

   prefs.Children.Add(new Border{Height=1,Background=Res("BorderBrush"),Margin=new Thickness(0,14,0,13)});

   var behavior=new Grid();
   behavior.ColumnDefinitions.Add(new ColumnDefinition{Width=new GridLength(1,GridUnitType.Star)});
   behavior.ColumnDefinitions.Add(new ColumnDefinition{Width=GridLength.Auto});
   var label=new StackPanel();
   label.Children.Add(Text("启动成功后自动关闭窗口",14.5,Res("Ink"),FontWeights.SemiBold));
   var note=Text("显示后台注入与 Lua 接管进度，成功进入游戏后倒计时 5 秒自动关闭。",12,Res("Muted"));
   note.Margin=new Thickness(0,5,20,0);label.Children.Add(note);behavior.Children.Add(label);
   var close=new CheckBox{Style=Style("Switch"),IsChecked=autoClose};
   close.Click+=delegate{try{autoClose=close.IsChecked==true;SavePreferences();}catch(Exception e){Status(e.Message,true);}};
   Grid.SetColumn(close,1);behavior.Children.Add(close);
   prefs.Children.Add(behavior);
   Get<StackPanel>("PageBody").Children.Add(Card(prefs));

   // Card 3: Shortcuts & Directory Tools
   var info=new StackPanel();
   info.Children.Add(Text("图片与快捷键说明",16,Res("Ink"),FontWeights.Bold));
   var detail=Text("• 自定义 PNG 立绘自动跟随游戏原图的实际尺寸、缩放、旋转与遮罩裁切关系。\n• 游戏中按 F10 重新扫描并热重载所有配置与图片，无需重启客户端。\n• 游戏中按 F11 统一关闭自定义立绘并恢复原图（再次按 F10 即可重新开启）。\n• 仅影响本机显示，角色与皮肤仍在游戏原有宿舍与大厅界面中选择。",12,Res("InkSecondary"));
   detail.Margin=new Thickness(0,8,0,13);detail.LineHeight=21;info.Children.Add(detail);
   var buttons=new StackPanel{Orientation=Orientation.Horizontal};
   buttons.Children.Add(Button("打开 Mod 文件夹",delegate{OpenFolder(modsRoot);}));
   var logs=Button("打开运行目录",delegate{OpenFolder(dataRoot);});logs.Margin=new Thickness(10,0,0,0);buttons.Children.Add(logs);
   info.Children.Add(buttons);
   Get<StackPanel>("PageBody").Children.Add(Card(info));
  }
  bool ChooseGame() {
   var chooser=new OpenFileDialog{Title="选择雀魂游戏启动程序",Filter="游戏程序 (*.exe)|*.exe",CheckFileExists=true};
   try{if(File.Exists(gamePath)){chooser.InitialDirectory=System.IO.Path.GetDirectoryName(gamePath);chooser.FileName=System.IO.Path.GetFileName(gamePath);}}catch{}
   if(chooser.ShowDialog(Window)!=true)return false;
   var path=System.IO.Path.GetFullPath(chooser.FileName);if(string.Equals(path,Assembly.GetExecutingAssembly().Location,StringComparison.OrdinalIgnoreCase))throw new IOException("请选择雀魂游戏程序，不要选择本工具。");
   var parent=System.IO.Path.GetDirectoryName(path);var game=File.Exists(System.IO.Path.Combine(parent,"GameAssembly.dll"))?parent:System.IO.Path.Combine(parent,"GameClient");
   if(!File.Exists(System.IO.Path.Combine(game,"GameAssembly.dll"))||!File.Exists(System.IO.Path.Combine(game,"UnityPlayer.dll"))||!File.Exists(System.IO.Path.Combine(game,"Jantama_MahjongSoul_Data","Plugins","x86","tolua.dll")))throw new IOException("此位置不是支持的雀魂客户端，请选择 Steam 安装目录或官方版 GameClient 中的游戏 EXE。");
   Files.Text(System.IO.Path.Combine(root,SettingsFileName),"game="+path+"\n");gamePath=path;return true;
  }
  void Import(Character initial) {
   var dialog=new Window{
    Title="添加自定义图片",Owner=Window,Icon=Window.Icon,
    Width=560,Height=670,MinHeight=630,
    WindowStartupLocation=WindowStartupLocation.CenterOwner,
    WindowStyle=WindowStyle.None,ResizeMode=ResizeMode.NoResize,
    Background=Res("ShellBg"),FontFamily=Window.FontFamily,FontSize=13,
    UseLayoutRounding=true,SnapsToDevicePixels=true
   };
   HookChrome(dialog);
   foreach(System.Collections.DictionaryEntry entry in Window.Resources)dialog.Resources[entry.Key]=entry.Value;

   var outer=new Border{CornerRadius=new CornerRadius(16),BorderBrush=Res("BorderStrongBrush"),BorderThickness=new Thickness(1),Background=Res("ShellBg")};
   var rootGrid=new Grid();
   rootGrid.RowDefinitions.Add(new RowDefinition{Height=new GridLength(44)});
   rootGrid.RowDefinitions.Add(new RowDefinition{Height=new GridLength(1,GridUnitType.Star)});
   outer.Child=rootGrid;dialog.Content=outer;

   var headerBar=new Border{Background=Res("TitleBarBg"),BorderBrush=Res("BorderBrush"),BorderThickness=new Thickness(0,0,0,1),CornerRadius=new CornerRadius(16,16,0,0)};
   headerBar.MouseLeftButtonDown+=delegate(object s,MouseButtonEventArgs e){if(e.ButtonState==MouseButtonState.Pressed)dialog.DragMove();};
   var headerGrid=new Grid();
   headerGrid.Children.Add(new TextBlock{Text="添加自定义立绘 Mod",FontSize=12,FontWeight=FontWeights.SemiBold,Foreground=Res("InkSecondary"),VerticalAlignment=VerticalAlignment.Center,Margin=new Thickness(20,0,0,0)});
   var closeBtn=new Button{Content="✕",Style=Style("CaptionCloseButton"),HorizontalAlignment=HorizontalAlignment.Right,VerticalAlignment=VerticalAlignment.Center,Margin=new Thickness(0,0,8,0)};
   closeBtn.Click+=delegate{dialog.Close();};
   headerGrid.Children.Add(closeBtn);
   headerBar.Child=headerGrid;
   rootGrid.Children.Add(headerBar);

   var body=new StackPanel{Margin=new Thickness(28,20,28,20)};
   Grid.SetRow(body,1);rootGrid.Children.Add(body);
   body.Children.Add(Text("为喜欢的皮肤，换上一张新图片",20,Res("Ink"),FontWeights.Bold));
   var note=Text("按角色与皮肤 ID 独立匹配，同一角色的其他皮肤保持原样。",12,Res("Muted"));
   note.Margin=new Thickness(0,6,0,16);body.Children.Add(note);

   body.Children.Add(Text("选择雀士角色",11.5,Res("InkSecondary"),FontWeights.SemiBold));
   var roles=new ComboBox{ItemsSource=Assets.Characters.Values.OrderBy(c=>c.Id).ToList(),Margin=new Thickness(0,6,0,13),SelectedItem=initial};
   body.Children.Add(roles);

   body.Children.Add(Text("选择目标皮肤",11.5,Res("InkSecondary"),FontWeights.SemiBold));
   var skins=new ComboBox{Margin=new Thickness(0,6,0,13)};
   body.Children.Add(skins);

   body.Children.Add(Text("Mod 文件夹名称",11.5,Res("InkSecondary"),FontWeights.SemiBold));
   var title=new TextBox{Text="自定义图片",Background=Res("CardSubtleBg"),BorderBrush=Res("BorderBrush"),BorderThickness=new Thickness(1),Foreground=Res("Ink"),Margin=new Thickness(0,6,0,14),MinHeight=38};
   body.Children.Add(title);

   var previewGrid=new Grid();
   var previewHint=Text("尚未选择 PNG 图片\n支持最大 4096 × 8192 透明 PNG",11.5,Res("Muted"));
   previewHint.TextAlignment=TextAlignment.Center;previewHint.HorizontalAlignment=HorizontalAlignment.Center;previewHint.VerticalAlignment=VerticalAlignment.Center;
   var preview=new Image{Stretch=Stretch.Uniform,MaxHeight=146,Margin=new Thickness(6)};
   previewGrid.Children.Add(previewHint);previewGrid.Children.Add(preview);
   var previewBorder=new Border{Height=156,CornerRadius=new CornerRadius(12),Background=Res("CardSubtleBg"),BorderBrush=Res("BorderBrush"),BorderThickness=new Thickness(1),Child=previewGrid};
   body.Children.Add(previewBorder);

   string chosen=null;byte[] png=null;
   var choose=Button("选择 PNG 图片…",delegate{
    try{
     var picker=new OpenFileDialog{Title="选择自定义 PNG",Filter="PNG 图片 (*.png)|*.png",CheckFileExists=true};
     if(picker.ShowDialog(dialog)!=true)return;
     png=Files.Png(picker.FileName);chosen=picker.FileName;
     preview.Source=Assets.Bitmap(png,450);
     previewHint.Visibility=Visibility.Collapsed;
    }catch(Exception e){MessageBox.Show(dialog,e.Message,"图片未导入",MessageBoxButton.OK,MessageBoxImage.Information);}
   });
   choose.Margin=new Thickness(0,12,0,12);body.Children.Add(choose);

   Action refreshSkins=delegate{
    var c=roles.SelectedItem as Character;if(c==null)return;
    skins.ItemsSource=c.Skins.ToList();
    skins.SelectedItem=c.Skins.FirstOrDefault(s=>s.Id==c.DefaultSkin)??c.Skins.FirstOrDefault();
   };
   roles.SelectionChanged+=delegate{refreshSkins();};if(initial==null)roles.SelectedIndex=0;refreshSkins();

   var save=Button("添加 Mod",delegate{
    try{
     var c=roles.SelectedItem as Character;var skin=skins.SelectedItem as Skin;if(c==null||skin==null)throw new IOException("请先选择角色与皮肤。");if(chosen==null||png==null)throw new IOException("请先选择一张 PNG 图片。");
     var name=title.Text.Trim().TrimEnd('.',' ');if(name.Length==0||name=="."||name==".."||name.IndexOfAny(System.IO.Path.GetInvalidFileNameChars())>=0)throw new IOException("请使用有效的 Mod 名称。");
     if(mods.Any(m=>m.Enabled&&m.Character==c.Id&&m.Skin==skin.Id))throw new IOException("这个皮肤已有启用的 Mod。请先关闭原 Mod，或直接使用“更换图片”。");
     var folder=System.IO.Path.Combine(modsRoot,c.Name,name);Files.Check(folder);if(Directory.Exists(folder))throw new IOException("此名称已存在，请换一个 Mod 名称。");
     Directory.CreateDirectory(folder);try{Files.Atomic(System.IO.Path.Combine(folder,"custom.png"),png);Files.Text(System.IO.Path.Combine(folder,"mod.ini"),"enabled = 1\ncharacter_id = "+c.Id+"\nskin_id = "+skin.Id+"\nimage = custom.png\n");}catch{if(!File.Exists(System.IO.Path.Combine(folder,"mod.ini"))){var image=System.IO.Path.Combine(folder,"custom.png");if(File.Exists(image))File.Delete(image);if(!Directory.EnumerateFileSystemEntries(folder).Any())Directory.Delete(folder);}throw;}
     ReadMods();selected=c;page="role";Render();Saved();dialog.Close();
    }catch(Exception e){MessageBox.Show(dialog,e.Message,"暂时无法添加",MessageBoxButton.OK,MessageBoxImage.Information);}
   },true);
   save.Height=44;body.Children.Add(save);dialog.ShowDialog();
  }
  void StartGame() {
   try{
    RefreshGame();if(launching)return;if(gameRunning){Status("游戏已经运行。请先退出，再从这里启动本地试用。",false);return;}
    if(!File.Exists(gamePath)&&!ChooseGame())return;
    var dll=System.IO.Path.Combine(root,DllFileName);if(!File.Exists(dll))throw new IOException("找不到旁边的 "+DllFileName+"，请完整解压后运行。");
    var engine=System.IO.Path.Combine(dataRoot,"launcher","engine.exe");byte[] payload;using(var stream=Assets.Resource("MJ.Engine"))using(var buffer=new MemoryStream()){stream.CopyTo(buffer);payload=buffer.ToArray();}
    Files.Check(engine);if(!File.Exists(engine)||!File.ReadAllBytes(engine).SequenceEqual(payload))Files.Atomic(engine,payload);
    launching=true;RefreshGame();Status("正在检查游戏路径…",false);Get<ProgressBar>("LaunchProgress").Visibility=Visibility.Visible;Get<ProgressBar>("LaunchProgress").Value=5;
    var messages=new StringBuilder();var errors=new StringBuilder();var logLock=new object();
    var worker=new Thread(delegate(){
     int exit=-1;string failure=null;
     try{
      using(var process=new Process()){
       process.StartInfo=new ProcessStartInfo{FileName=engine,Arguments="--headless --root "+Files.Quote(root)+" --game "+Files.Quote(gamePath)+" --close-after 0",WorkingDirectory=root,UseShellExecute=false,CreateNoWindow=true,WindowStyle=ProcessWindowStyle.Hidden,RedirectStandardInput=true,RedirectStandardOutput=true,RedirectStandardError=true,StandardOutputEncoding=Encoding.UTF8,StandardErrorEncoding=Encoding.UTF8};
       process.OutputDataReceived+=delegate(object sender,DataReceivedEventArgs e){if(e.Data==null)return;lock(logLock)messages.AppendLine(e.Data);Window.Dispatcher.BeginInvoke(new Action(delegate{Progress(e.Data);}));};
       process.ErrorDataReceived+=delegate(object sender,DataReceivedEventArgs e){if(e.Data==null)return;lock(logLock){errors.AppendLine(e.Data);messages.AppendLine(e.Data);}};
       process.Start();process.StandardInput.Close();process.BeginOutputReadLine();process.BeginErrorReadLine();process.WaitForExit();exit=process.ExitCode;
      }
     }catch(Exception e){failure=e.Message;}
     try{lock(logLock)Files.Text(System.IO.Path.Combine(dataRoot,"launcher","launch.log"),messages.ToString());}catch{}
     var code=exit;var error=failure;string detail;lock(logLock)detail=errors.ToString().Trim();
     Window.Dispatcher.BeginInvoke(new Action(delegate{
      launching=false;RefreshGame();
      if(code!=0){Get<ProgressBar>("LaunchProgress").Visibility=Visibility.Collapsed;Status("启动未完成："+(error??(detail.Length>0?detail:"请查看运行目录中的启动日志。")),true);return;}
      if(!connected){Get<ProgressBar>("LaunchProgress").Visibility=Visibility.Collapsed;Status("游戏已启动，暂未确认本地试用加载完成。请稍候，或查看启动日志。",true);return;}
      Get<ProgressBar>("LaunchProgress").Value=100;Status("游戏已启动，本地试用已准备就绪。",false);
      if(autoClose){closeSeconds=closeAfterSeconds;Get<Button>("KeepOpenButton").Visibility=Visibility.Visible;closeTimer=new DispatcherTimer{Interval=TimeSpan.FromSeconds(1)};closeTimer.Tick+=delegate{closeSeconds--;if(closeSeconds<=0){closeTimer.Stop();Window.Close();}else Status("游戏已启动，"+closeSeconds+" 秒后自动关闭窗口…",false);};closeTimer.Start();}
     }));
    });worker.IsBackground=true;worker.Start();
   }catch(Exception e){launching=false;RefreshGame();Status(e.Message,true);}
  }
  void Progress(string line) {
   if(!launching)return;var bar=Get<ProgressBar>("LaunchProgress");
   if(line.Contains("已选择")){bar.Value=18;Status("正在检查客户端…",false);}
   else if(line.Contains("进程已准备")){bar.Value=35;Status("正在启动游戏…",false);}
   else if(line.Contains("DLL 已加载")){bar.Value=65;Status("本地试用已加载，正在准备游戏…",false);}
   else if(line.Contains("Steam 客户端保持运行")){bar.Value=90;Status("正在确认 Steam 版游戏运行状态…",false);}
   else if(line.Contains("Lua 入口已接管")){bar.Value=100;Status("启动成功，正在进入游戏。",false);}
   else if(line.Contains("暂未确认"))Status("游戏正在运行，等待本地试用完成初始化…",false);
  }
  public void Arguments(string[] args) {
   bool pick=false,start=false;
   for(int i=0;i<args.Length;i++){
    if(args[i]=="--pick"||args[i]=="--choose-game"){pick=true;start=true;}
    else if(args[i]=="--game"){if(++i>=args.Length)throw new IOException("--game 需要游戏路径。");gamePath=System.IO.Path.GetFullPath(args[i]);start=true;}
    else if(args[i]=="--close-after"){int seconds;if(++i>=args.Length||!int.TryParse(args[i],out seconds)||seconds<0||seconds>3600)throw new IOException("--close-after 需要 0 到 3600 的秒数。");autoClose=seconds!=0;if(seconds>0)closeAfterSeconds=seconds;}
    else if(args[i]=="--theme"){if(++i>=args.Length)throw new IOException("--theme 需要 light 或 dark。");ApplyTheme(args[i],true);}
    else throw new IOException("未知参数："+args[i]);
   }
   if(start){bool done=false;Window.ContentRendered+=delegate{if(done)return;done=true;try{if(pick&&!ChooseGame())return;StartGame();}catch(Exception e){Status(e.Message,true);}};}
  }
  public void ExportSnapshots(string directory) {
   Directory.CreateDirectory(directory);
   var rootBorder=Get<Border>("RootBorder");
   rootBorder.Width=1260;rootBorder.Height=830;
   foreach(var mode in new[]{"light","dark"}){
    ApplyTheme(mode,false);
    foreach(var view in new[]{"characters","role","settings"}){
     if(view=="role"){selected=Assets.Characters[200017];page="role";Render();}
     else Navigate(view);
     rootBorder.Measure(new Size(1260,830));
     rootBorder.Arrange(new Rect(0,0,1260,830));
     rootBorder.UpdateLayout();
     var rtb=new RenderTargetBitmap(1260,830,96,96,PixelFormats.Pbgra32);
     rtb.Render(rootBorder);
     var encoder=new PngBitmapEncoder();
     encoder.Frames.Add(BitmapFrame.Create(rtb));
     using(var fs=File.Create(System.IO.Path.Combine(directory,"preview-"+mode+"-"+view+".png")))encoder.Save(fs);
    }
   }
  }
  public string SelfTest() {
   var checks=new List<string>();Action<bool,string> assert=delegate(bool ok,string message){if(!ok)throw new Exception(message);checks.Add(message);};
   assert(Assets.Characters.Count>=125,"embedded character catalog");assert(Assets.Characters[200017].Skins.Any(s=>s.Id==401701)&&Assets.Characters[200017].Skins.Any(s=>s.Id==401710),"default and Pure White Afternoon skin IDs");
   assert(Assets.Characters.Values.Sum(c=>c.Skins.Count)>=514,"embedded skin catalog");assert(Assets.Avatar(200017)!=null,"embedded avatar without external resource folder");
   var updatedRoles=new Dictionary<uint,uint[]> {
    {20000127,new uint[]{40012701,40012702}},
    {20000128,new uint[]{40012801,40012802,40012803}},
    {20000129,new uint[]{40012901,40012902,40012903}},
    {20000130,new uint[]{40013001,40013002,40013003}},
    {20000131,new uint[]{40013101,40013102,40013103}}
   };
   foreach(var role in updatedRoles){
    Character character;
    assert(Assets.Characters.TryGetValue(role.Key,out character)&&Assets.Avatar(role.Key)!=null,"embedded character and avatar "+role.Key);
    assert(character.DefaultSkin==role.Value[0]&&character.Skins.Select(s=>s.Id).SequenceEqual(role.Value),"import skin options for "+role.Key);
   }
   using(var resource=Assets.Resource("MJ.Engine")){assert(resource.ReadByte()=='M'&&resource.ReadByte()=='Z',"embedded native launch engine");}
   assert(Window.Title=="Majsoul-modLauncher"&&Window.FindName("NavMods")==null,"launcher title and navigation");
   foreach(var name in new[]{"LaunchButton","PageBody","NavCharacters","NavSettings","SearchBox","EnableAllModsButton","DisableAllModsButton","ReloadButton","RestoreButton","ThemeLightButton","ThemeDarkButton"})assert(Window.FindName(name)!=null,"window control "+name);
   var test=System.IO.Path.Combine(root,"target","gui-selftest-"+Guid.NewGuid().ToString("N"));Directory.CreateDirectory(test);
   try{
    var ini=System.IO.Path.Combine(test,"中文角色","纯白午后","mod.ini");Files.Text(ini,"# keep the image and layout\nenabled = 1\ncharacter_id = 200017\nskin_id = 401710\nimage = 测试.png\ndorm_x = -12.5\n");
    var example=new Mod{Ini=ini,Enabled=true};SetEnabled(example,false);var v=Files.Ini(ini);assert(v["enabled"]=="0"&&v["image"]=="测试.png"&&v["dorm_x"]=="-12.5"&&Files.Id(v,"skin_id","skin",0)==401710,"toggle preserves IDs, Unicode image path and layout");
    SetEnabled(example,true);assert(Files.Ini(ini)["enabled"]=="1","reenable mod atomically");
    Files.Text(ini,"character=200017\ncharacter_id=200017\n");bool rejected=false;try{Files.Id(Files.Ini(ini),"character_id","character",0);}catch(IOException){rejected=true;}assert(rejected,"reject duplicate alias IDs");
    assert(Files.Quote("D:\\角色 名称\\")=="\"D:\\角色 名称\\\\\"","Windows argument quoting with spaces and trailing separator");
    var art=System.IO.Path.Combine(test,"头像.png");byte[] avatar;using(var zip=new ZipArchive(Assets.Resource("MJ.Assets"),ZipArchiveMode.Read))using(var stream=zip.GetEntry("avatars/200017.png").Open())using(var buffer=new MemoryStream()){stream.CopyTo(buffer);avatar=buffer.ToArray();}Files.Atomic(art,avatar);assert(Files.Png(art).SequenceEqual(avatar),"PNG validation preserves bytes");
    var legacy=System.IO.Path.Combine(test,"MJLocalPreview");
    Files.Text(System.IO.Path.Combine(legacy,"launcher","ui.ini"),"theme=dark\nclose_after_launch=0\n");
    var legacySettings=System.IO.Path.Combine(test,LegacySettingsFileName);
    var currentSettings=System.IO.Path.Combine(test,SettingsFileName);
    var rememberedGame=@"D:\雀魂 测试\Jantama_MahjongSoul.exe";
    var savedSettings=Encoding.UTF8.GetBytes("\uFEFFgame="+rememberedGame+"\r\n");
    Files.Atomic(legacySettings,savedSettings);
    var batch=new Shell(test);
    try{
     assert(batch.dataRoot==System.IO.Path.Combine(test,"MaJmods")&&!Directory.Exists(legacy)&&batch.theme=="dark"&&!batch.autoClose,"legacy data directory moves before loading launcher preferences");
     assert(batch.gamePath==rememberedGame&&!File.Exists(legacySettings)&&File.ReadAllBytes(currentSettings).SequenceEqual(savedSettings),"legacy game path migrates with its original bytes");
     Files.Text(legacySettings,@"game=D:\旧客户端\Jantama_MahjongSoul.exe");
     batch.LoadPreferences();
     assert(batch.gamePath==rememberedGame&&File.Exists(legacySettings),"existing MaJmods.ini takes precedence over legacy settings");
     var enable=batch.Get<Button>("EnableAllModsButton");
     var disable=batch.Get<Button>("DisableAllModsButton");
     var click=System.Windows.Controls.Primitives.ButtonBase.ClickEvent;
     assert(!enable.IsEnabled&&!disable.IsEnabled,"bulk actions disabled when no mods are installed");
     var alternate=System.IO.Path.Combine(batch.modsRoot,"角色","A","mod.ini");
     var active=System.IO.Path.Combine(batch.modsRoot,"角色","B","mod.ini");
     var otherSkin=System.IO.Path.Combine(batch.modsRoot,"角色","C","mod.ini");
     var otherRole=System.IO.Path.Combine(batch.modsRoot,"另一角色","D","mod.ini");
     Files.Text(alternate,"enabled=0\ncharacter=200017\nskin=401701\n");
     Files.Text(active,"character_id=200017\nskin_id=401701\n");
     Files.Text(otherSkin,"enabled=0\ncharacter_id=200017\nskin_id=401710\ndorm_x=-12.5\n");
     Files.Text(otherRole,"enabled=0\ncharacter_id=200038\n");
     batch.ReadMods();
     batch.Get<TextBox>("SearchBox").Text="200017";
     enable.RaiseEvent(new RoutedEventArgs(click));
     assert(batch.mods.Count(mod=>mod.Enabled)==3&&Files.Ini(alternate)["enabled"]=="0"&&!Files.Ini(active).ContainsKey("enabled")&&Files.Ini(otherRole)["enabled"]=="1","bulk enable includes filtered-out characters and retains the enabled mod per skin");
     using(var locked=new FileStream(otherSkin,FileMode.Open,FileAccess.Read,FileShare.Read)){
      disable.RaiseEvent(new RoutedEventArgs(click));
      assert(batch.mods.Count(mod=>mod.Enabled)==1&&Files.Ini(otherSkin)["enabled"]=="1"&&batch.statusIsError,"bulk disable reports a locked file and saves the other mods");
     }
     disable.RaiseEvent(new RoutedEventArgs(click));
     assert(batch.mods.All(mod=>!mod.Enabled)&&Files.Ini(active)["enabled"]=="0"&&Files.Ini(otherSkin)["dorm_x"]=="-12.5","bulk disable persists and preserves other settings");
     enable.RaiseEvent(new RoutedEventArgs(click));
     assert(Files.Ini(alternate)["enabled"]=="1"&&Files.Ini(active)["enabled"]=="0","bulk enable chooses a consistent mod when all alternatives are disabled");
    }finally{batch.Window.Close();}
   }finally{if(!test.StartsWith(System.IO.Path.Combine(root,"target")+"\\",StringComparison.OrdinalIgnoreCase))throw new Exception("invalid test cleanup path");Directory.Delete(test,true);}
   string originalTheme=theme;
   ApplyTheme("dark",false);assert(theme=="dark"&&Get<TextBlock>("FooterThemeText").Text.Contains("深色"),"dark theme switch");
   ApplyTheme("light",false);assert(theme=="light"&&Get<TextBlock>("FooterThemeText").Text.Contains("浅色"),"light theme switch");
   ApplyTheme(originalTheme,false);
   selected=Assets.Characters[200017];page="role";Render();assert(Get<StackPanel>("PageBody").Children.Count>0,"character detail renders existing mods");page="settings";Render();assert(Get<StackPanel>("PageBody").Children.Count==3,"settings page renders");Navigate("characters");
   Get<TextBox>("SearchBox").Text="200017";assert(((WrapPanel)Get<StackPanel>("PageBody").Children[0]).Children.Count==2,"search by character ID");Get<TextBox>("SearchBox").Text="二之宫花";assert(((WrapPanel)Get<StackPanel>("PageBody").Children[0]).Children.Count==2,"search by Chinese character name");Get<TextBox>("SearchBox").Text="no-such-character";assert(((WrapPanel)Get<StackPanel>("PageBody").Children[0]).Children.Count==1,"empty search result");Get<TextBox>("SearchBox").Text="";
   return "PASS\n"+string.Join("\n",checks)+"\nInstalled mods: "+mods.Count+"\n";
  }
 }
 internal static class Program {
  [DllImport("user32.dll",CharSet=CharSet.Unicode)]static extern IntPtr FindWindow(string cls,string title);
  [DllImport("user32.dll")]static extern bool SetForegroundWindow(IntPtr window);
  [DllImport("user32.dll")]static extern bool ShowWindow(IntPtr window,int command);
  [STAThread] public static int Main(string[] args) {
   string report=args.Length==2&&args[0]=="--self-test"?args[1]:null;
   string snapshots=args.Length==2&&args[0]=="--export-snapshots"?args[1]:null;
   try{
    if(args.Length==1&&(args[0]=="--help"||args[0]=="-h")){Console.WriteLine("Majsoul-modLauncher 0.6.9\n双击打开角色卡片式启动器（支持浅色/深色两种主题方案）。\n--pick / --choose-game 重新选择游戏并启动\n--game <EXE路径> 指定游戏并启动\n--close-after <秒> 自动关闭倒计时；0 保留窗口\n--theme <light|dark> 切换浅色或深色主题\nF10 重载；F11 恢复原图。");return 0;}
    var root=System.IO.Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location);
    if(report!=null){var app=new Application();var shell=new Shell(root);File.WriteAllText(report,shell.SelfTest(),Encoding.UTF8);shell.Window.Close();app.Shutdown();return 0;}
    if(snapshots!=null){var app=new Application();var shell=new Shell(root);shell.ExportSnapshots(snapshots);shell.Window.Close();app.Shutdown();return 0;}
    bool created;using(var mutex=new Mutex(true,"Local\\MJLocalPreview.Gui",out created)){
     if(!created){var window=FindWindow(null,"Majsoul-modLauncher");if(window==IntPtr.Zero)window=FindWindow(null,"雀魂 · 本地装扮试用");if(window!=IntPtr.Zero){ShowWindow(window,9);SetForegroundWindow(window);}return 0;}
     var app=new Application();app.DispatcherUnhandledException+=delegate(object sender,DispatcherUnhandledExceptionEventArgs e){MessageBox.Show(e.Exception.Message,"暂时无法完成操作",MessageBoxButton.OK,MessageBoxImage.Information);e.Handled=true;};var shell=new Shell(root);shell.Arguments(args);app.Run(shell.Window);
    }return 0;
   }catch(Exception e){if(report!=null)File.WriteAllText(report,"FAIL\n"+e,Encoding.UTF8);else MessageBox.Show(e.Message,"启动器无法打开",MessageBoxButton.OK,MessageBoxImage.Error);return 1;}
  }
 }
}
