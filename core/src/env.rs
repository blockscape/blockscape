use std::fs;
use std::path::{Path,PathBuf};
use std::env::home_dir;

pub fn prepare_storage_dir() {
    let p = get_storage_dir();
    
    match p {
        Some(dir) => {

            ensure_mkdir(dir.as_path());
            ensure_mkdir(dir.join("keys").as_path());
            ensure_mkdir(dir.join("db").as_path());
        },
        None => panic!("Could not find storage directory! Please check your environment and try again.")
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_storage_dir() -> Option<PathBuf> {
    let d = home_dir();

    match d {
        Some(mut d) => {
            d.push(".blockscape");
            Some(d)
        },
        None => None
    }
}

fn ensure_mkdir(p: &Path) {
    if !p.is_dir() {

        info!("Create storage directory: {}", p.display());

        match fs::create_dir(p) {
            Ok(_) => return,
            Err(why) => panic!("Could not create storage directory! Please check {} for access and try again. {:?}", p.display(), why.kind())
        }
    }
}

// NOTE: Build currently fails on windows until we tell it how to save