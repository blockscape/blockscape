use std::fs;
use std::io::{Write, Read};
use std::path::{Path,PathBuf};
use std::env::home_dir;

use openssl::pkey::PKey;

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

fn gen_key_path(name: &str) -> Box<Path> {
    let mut pb = get_storage_dir().unwrap();
    
    pb.push("keys");
    pb.push(format!("{}.pem", name));
    
    pb.into_boxed_path()
}

/// TODO: Needs proper error handling!
pub fn save_key(name: &str, key: &PKey) -> bool {
    let p = gen_key_path(name);

    // should basically always succeed
    if let Ok(pem) = key.private_key_to_pem() {
        if let Ok(mut f) = fs::File::create(&p) {
            if f.write_all(&pem).is_ok() {
                return true;
            }
        }
    }

    false
}

// TODO: Again, proper error handling needed
pub fn load_key(name: &str) -> Option<PKey> {
    let p = gen_key_path(name);

    if let Ok(mut f) = fs::File::open(&p) {
        let mut buf: Vec<u8> = Vec::new();

        if f.read_to_end(&mut buf).is_ok() {
            // parse pem
            if let Ok(key) = PKey::private_key_from_pem(&buf) {
                return Some(key);
            }
        }
    }

    None
}

// NOTE: Build currently fails on windows until we tell it how to save