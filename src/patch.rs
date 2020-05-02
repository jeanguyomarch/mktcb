/* This is part of mktcb - which is under the MIT License ********************/

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
