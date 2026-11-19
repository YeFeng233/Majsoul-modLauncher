//! Steam installation discovery and the environment for a directly launched client.
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    os::windows::ffi::OsStrExt,
    path::Path,
};

use crate::launcher::{self, Result};

const APP_IDS: [u32; 3] = [1329410, 2717220, 2739990];

fn manifest_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("\"{key}\"");
    let mut values = text.lines().filter_map(|line| {
        line.trim()
            .strip_prefix(&prefix)?
            .trim()
            .strip_prefix('"')?
            .strip_suffix('"')
    });
    let value = values.next()?;
    values.next().is_none().then_some(value)
}

pub fn app_id(exe: &Path) -> Result<Option<u32>> {
    let game = exe.parent().ok_or("无法读取游戏目录")?;
    if !game
        .join("Jantama_MahjongSoul_Data/Plugins/x86/steam_api.dll")
        .is_file()
    {
        return Ok(None);
    }
    let Some(common) = game.parent() else {
        return Ok(None);
    };
    let Some(steamapps) = common.parent() else {
        return Ok(None);
    };
    if !common.file_name().is_some_and(|n| n.eq_ignore_ascii_case("common"))
        || !steamapps
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("steamapps"))
    {
        return Ok(None);
    }
    let folder = game.file_name().ok_or("无法读取 Steam 安装目录")?;
    for id in APP_IDS {
        let manifest = steamapps.join(format!("appmanifest_{id}.acf"));
        if !manifest.is_file() {
            continue;
        }
        launcher::unlinked(&manifest)?;
        if fs::metadata(&manifest).map_err(|e| e.to_string())?.len() > 131072 {
            return Err("Steam 安装记录过大，请通过 Steam 检查游戏安装".into());
        }
        let text = fs::read_to_string(&manifest).map_err(|e| e.to_string())?;
        if !manifest_value(&text, "installdir")
            .is_some_and(|name| folder.eq_ignore_ascii_case(name))
        {
            continue;
        }
        if manifest_value(&text, "appid").and_then(|value| value.parse().ok()) != Some(id) {
            return Err("Steam 安装记录的应用编号不一致，请通过 Steam 检查游戏安装".into());
        }
        return Ok(Some(id));
    }
    Err("找不到该雀魂客户端的 Steam 安装记录，请通过 Steam 完成安装或更新".into())
}

fn environment_block(
    id: u32,
    inherited: impl IntoIterator<Item = (OsString, OsString)>,
) -> Vec<u16> {
    // Windows treats environment variable names case-insensitively. Preserve
    // inherited values, including Unicode and per-drive working directories.
    let mut entries = BTreeMap::new();
    let overrides = [
        (OsString::from("SteamAppId"), OsString::from(id.to_string())),
        (OsString::from("SteamGameId"), OsString::from(id.to_string())),
    ];
    for (key, value) in inherited.into_iter().chain(overrides) {
        let sort_key: Vec<u16> = key
            .encode_wide()
            .map(|c| if (97..=122).contains(&c) { c - 32 } else { c })
            .collect();
        entries.insert(sort_key, (key, value));
    }
    let mut block = Vec::new();
    for (key, value) in entries.values() {
        block.extend(key.encode_wide());
        block.push(b'=' as u16);
        block.extend(value.encode_wide());
        block.push(0);
    }
    block.push(0);
    block
}

pub fn environment(exe: &Path) -> Result<Option<Vec<u16>>> {
    Ok(app_id(exe)?.map(|id| environment_block(id, std::env::vars_os())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_environment_preserves_unicode_and_replaces_inherited_steam_ids() {
        let inherited = [
            ("PATH", "C:\\工具\\bin"),
            ("sTeAmApPiD", "999"),
            ("STEAMGAMEID", "888"),
            ("=C:", "C:\\工作目录"),
        ]
        .map(|(key, value)| (OsString::from(key), OsString::from(value)));
        let block = environment_block(1329410, inherited);
        assert_eq!(&block[block.len() - 2..], &[0, 0]);
        let text = String::from_utf16(&block).unwrap();
        let entries: Vec<_> = text.split('\0').filter(|s| !s.is_empty()).collect();
        assert_eq!(
            entries,
            [
                "=C:=C:\\工作目录",
                "PATH=C:\\工具\\bin",
                "SteamAppId=1329410",
                "SteamGameId=1329410"
            ]
        );
    }

    #[test]
    fn steam_detection_requires_matching_manifest_and_installed_api() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("steam-context-test-{}", std::process::id()));
        let game = root.join("steamapps/common/雀魂 测试");
        let plugins = game.join("Jantama_MahjongSoul_Data/Plugins/x86");
        fs::create_dir_all(&plugins).unwrap();
        let exe = game.join("Jantama_MahjongSoul.exe");
        let manifest = root.join("steamapps/appmanifest_1329410.acf");
        fs::write(
            &manifest,
            "\"AppState\"\n{\n\"appid\" \"1329410\"\n\"installdir\" \"雀魂 测试\"\n}\n",
        )
        .unwrap();
        assert_eq!(app_id(&exe).unwrap(), None);
        fs::write(plugins.join("steam_api.dll"), b"test").unwrap();
        assert_eq!(app_id(&exe).unwrap(), Some(1329410));
        fs::write(
            &manifest,
            "\"appid\" \"2739990\"\n\"installdir\" \"雀魂 测试\"\n",
        )
        .unwrap();
        assert!(app_id(&exe).is_err());
        fs::write(
            &manifest,
            "\"appid\" \"1329410\"\n\"installdir\" \"Another Game\"\n",
        )
        .unwrap();
        assert!(app_id(&exe).is_err());
        assert!(root.starts_with(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")));
        fs::remove_dir_all(root).unwrap();
    }
}
