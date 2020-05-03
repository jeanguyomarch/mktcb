/* This is part of mktcb - which is under the MIT License ********************/

use snafu::{ResultExt, OptionExt};

use std::path::PathBuf;
use crate::error::Result;
use crate::error;
use log::*;

/// Retrieve the last path component of an URL, as a PathBuf
pub fn url_last(url: &url::Url) -> Result<PathBuf> {
    let filename = url.path_segments()
        .context(error::URLExtractError{url: url.clone()})?
        .last()
        .context(error::URLExtractError{url: url.clone()})?;
    Ok(std::path::PathBuf::from(filename))
}

pub fn copy_config(opt_cfg: &Option<PathBuf>, build_dir: &PathBuf) -> Result<()> {
    // Let create the build directory. We will need it anyway.
    std::fs::create_dir_all(build_dir).context(
        error::CreateDirError{ path: build_dir.clone() })?;
    if let Some(cfg) = opt_cfg {
        let mut build_cfg = build_dir.clone();
        build_cfg.push(".config");
        info!("Copying configuration {:#?} to {:#?}", cfg, build_cfg);
        std::fs::copy(cfg, &build_cfg).context(error::CopyFailed{
            from: cfg.clone(),
            to: build_cfg,
        })?;
    } else {
        debug!("No configuration selected");
    }
    Ok(())
}
