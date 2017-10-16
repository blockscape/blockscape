use std::fs;
use std::path::{Path,PathBuf};
use std::env::home_dir;

/// Create the storage directory structure if it does not exist, and make sure it is valid if it
/// does.
pub fn prepare_storage_dir(p: &PathBuf) {
    ensure_mkdir(p.as_path());
    ensure_mkdir(p.join("keys").as_path());
    ensure_mkdir(p.join("db").as_path());
    ensure_mkdir(p.join("nodes").as_path());
}

/// This returns the storage directory for blockscape on unix.
/// TODO: handle failure condition here, because if we cannot open the storage directory, then the
/// program cannot really run.
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

/// Return the formal name of this executable.
pub fn get_client_name() -> String {
    // TODO: Make more intelligent
    "Blockscape Official v".to_owned() + env!("CARGO_PKG_VERSION")
}

/// Make sure the directory exists. If it is there, this is a nop, if it is not, then it creates the
/// directory.
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