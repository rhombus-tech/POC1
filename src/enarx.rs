// SPDX-License-Identifier: Apache-2.0

use lazy_static::lazy_static;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use home;

const ENARX_REPO: &str = "enarx";
const ENARX_COMMIT: &str = "main"; // Or a specific commit if needed

lazy_static! {
    pub static ref ENARX_SDK: String = {
        println!("cargo:rerun-if-env-changed=CARGO_ENARX_SDK"); // Use a more common name
        env::var("CARGO_ENARX_SDK").unwrap_or_else(|_| {
            find_cargo_git_checkout(ENARX_REPO, ENARX_COMMIT)
                .expect("Could not find enarx checkout, environment variable CARGO_ENARX_SDK not set")
        })
    };
}

pub fn enarx_sdk() -> PathBuf {
    Path::new(&ENARX_SDK.to_string()).to_path_buf()
}

pub fn enarx_include_paths() -> Vec<PathBuf> {
    // Adapt this to the actual include paths in your enarx repo
    vec![
        enarx_sdk().join("src/include"),
        enarx_sdk().join("targets/x86_64-unknown-linux-gnu/include"),
    ]
}

fn find_cargo_git_checkout<'a>(crate_name: &'a str, commit: &'a str) -> Option<String> {
    let git_checkouts = home::cargo_home().ok()?.join("git/checkouts");

    fs::read_dir(git_checkouts)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_str().map(|s| s.starts_with(crate_name)).unwrap_or(false))
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let checkout = entry.path();
            // Important: Look for the specific folder containing the commit.
            match checkout.join("main").read_dir().ok() { // Look for `main` branch (or the folder named like the commit)
                Ok(iter) => iter
                    .filter_map(Result::ok)
                    .filter(|entry| entry.file_name().to_str().unwrap_or("") == commit) // Check for the commit
                    .filter(|entry| entry.path().is_dir())
                    .map(|entry| entry.path().canonicalize().ok())
                    .next(),
                Err(_) => None,
            }
        })
        .flatten()
        .map(|canonicalized_path| canonicalized_path.to_str().unwrap().to_string())
        .next()
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_find_enarx_checkout() {
        let result = find_cargo_git_checkout("enarx", "main");
        
        assert!(result.is_some()); // Or assert for specific content if known.
    }
}
