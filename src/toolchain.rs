/* This is part of mktcb - which is under the MIT License ********************/

use snafu::{ResultExt, OptionExt, ensure};
use crate::error::Result;
use crate::error;
use crate::download;
use crate::decompress;

use log::*;
use std::path::PathBuf;

pub fn fetch(url: &url::Url, dir: &PathBuf) -> Result<PathBuf> {
    // Compose the path to the tar archive to be downloaded
    let filename = url.path_segments()
        .context(error::URLExtractError{url: url.clone()})?
        .last()
        .context(error::URLExtractError{url: url.clone()})?;
    let mut path = dir.clone();
    path.push(filename);


    // We suppose that the result after extraction will be the name of the
    // archive stripped from its extensions (in practise, that's what is
    // done, but this is unsafe).
    let mut untar_dir = path.clone();
    untar_dir.set_extension(""); /* Strip compression extension */
    untar_dir.set_extension(""); /* Strip .tar */

    // If the directory containing the toolchain does not exist, download
    // and decompress it. Otherwise, skip this part!
    if ! untar_dir.is_dir() {
        info!("Downloading toolchain from {:#?}", url);
        let mut http_handle = curl::easy::Easy::new();
        download::to_file(&mut http_handle, &url, &path)?;
        let tc_dir = decompress::untar(&path)?;
        assert!(tc_dir == untar_dir);
        Ok(tc_dir)
    } else {
        Ok(untar_dir)
    }
}
