//! Defaults are also compiled into the DLL; the delivery needs no data directory.
use std::{fs, io::Write, path::Path};
use crate::launcher::{self, Result};

pub const FILES: &[(&str, &[u8])] = &[
    ("runtime.lua", include_bytes!("../data/runtime.lua")),
    ("adapter.tsv", include_bytes!("../data/adapter.tsv")),
    ("config.ini", include_bytes!("../data/config.ini")),
    ("max_data.yaml", include_bytes!("../data/max_data.yaml")),
];

pub fn prepare(base: &Path, legacy: Option<&Path>) -> Result<()> {
    launcher::unlinked(base)?;
    fs::create_dir_all(base).map_err(|e| e.to_string())?;
    for (name, bytes) in FILES {
        let path=base.join(name);
        launcher::unlinked(&path)?;
        if path.exists() { continue; }
        // Import the existing catalogue/settings once, without touching skin memory.
        let previous=legacy.map(|p|p.join(name)).filter(|p|p.is_file() &&
            (*name=="max_data.yaml" || *name=="config.ini"));
        let content=if let Some(p)=previous {launcher::unlinked(&p)?;fs::read(p).map_err(|e|e.to_string())?} else {bytes.to_vec()};
        match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file)=>file.write_all(&content).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?,
            Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>{},
            Err(e)=>return Err(format!("无法准备 {}：{e}",path.display())),
        }
    }
    Ok(())
}
