/* This is part of mktcb - which is under the MIT License ********************/

use snafu::ResultExt;
use crate::error::Result;
use crate::error;
use crate::config::Config;
use crate::download;
use crate::util;

use log::*;
use std::path::PathBuf;

pub struct Toolchain {
    pub cross_compile: String,
    url: url::Url,
    target_dir: PathBuf,
    download_dir: PathBuf,
}

impl Toolchain {
    pub fn fetch(&self) -> Result<()> {
        // If the directory containing the toolchain does not exist, download
        // and decompress it. Otherwise, skip this part!
        if ! self.target_dir.is_dir() {
            info!("Downloading toolchain from {:#?}", self.url);
            let mut http_handle = curl::easy::Easy::new();
            download::to_unpacked_dir(
                &mut http_handle, &self.url, &self.download_dir, &self.target_dir)?;
        }
        Ok(())
    }
}

pub fn new(config: &Config) -> Result<Toolchain> {
    let url = url::Url::parse(&config.toolchain.url)
        .context(error::InvalidToolchainURL{})?;

    // Compose the path to the tar archive to be downloaded
    let mut tar_path = config.download_dir.clone();
    tar_path.push(util::url_last(&url)?);

    // We suppose that the result after extraction will be the name of the
    // archive stripped from its extensions (in practise, that's what is
    // done, but this is unsafe).
    let mut untar_dir = tar_path.clone();
    untar_dir.set_extension(""); /* Strip compression extension */
    untar_dir.set_extension(""); /* Strip .tar */

    // Finally, compose the full cross-compile variable.
    let mut cc = untar_dir.clone();
    cc.push(config.toolchain.cross_compile.clone());

    Ok(Toolchain {
        cross_compile: cc.to_str().unwrap().to_string(),
        url: url,
        target_dir: untar_dir,
        download_dir: config.download_dir.clone(),
    })
}
