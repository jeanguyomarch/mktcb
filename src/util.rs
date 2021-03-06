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

pub fn getenv(var: &str) -> Result<String> {
    std::env::var(var).context(error::MaintainerError{ var: var.to_string() })
}

pub fn read_file(path: &std::path::PathBuf) -> Result<String> {
    let contents = std::fs::read(&path).context(
        error::FailedToReadVersion { path: path.clone() }
    )?;
    let mut data = std::string::String::from_utf8(contents)
        .context(error::FailedToDecodeUTF8{})?;
    // Right-trim the string from any whitespaces (including newlines)
    while let Some(idx) = data.rfind(char::is_whitespace) {
        data.truncate(idx);
    }
    Ok(data)
}
