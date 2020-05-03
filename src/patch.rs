/* This is part of mktcb - which is under the MIT License ********************/

use std::path::PathBuf;

use crate::error::Result;
use crate::error;
use log::*;

use snafu::{ResultExt, ensure};

use std::process::{Command, Stdio};

/// Run the patch command to apply a diff to a source tree 'working_dir'
pub fn patch(working_dir: &std::path::PathBuf, diff: &std::path::PathBuf) -> Result<()> {
    debug!("Applying patch {:#?} on {:#?}", diff, working_dir);
    let status = Command::new("patch")
        .current_dir(working_dir)
        .arg("-s") // Silent patch
        .arg("-p1")
        .arg("-i").arg(diff)
        .stdin(Stdio::null())
        .status()
        .context(error::ProgFailed{ proc: "patch".to_string() })?;
    ensure!(status.success(), error::PatchFailed{ path: working_dir.clone() });
    Ok(())
}

pub fn apply_patches_in(patches_dir: &PathBuf, source_dir: &PathBuf) -> Result<()> {
    if patches_dir.is_dir() {
        let dir_iter = std::fs::read_dir(&patches_dir)
            .context(error::DirIterFailed{dir: patches_dir.clone()})?;
        for dir_it in dir_iter {
            let entry = dir_it
                .context(error::DirIterFailed{dir: patches_dir.clone()})?
                .path();
            if entry.is_file() {
                patch(&source_dir, &entry)?;
            }
        }
    }
    Ok(())
}
