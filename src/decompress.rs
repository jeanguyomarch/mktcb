/* This is part of mktcb - which is under the MIT License ********************/

// Traits ---------------------------------------------------------------------
use std::io::Read;
use std::io::Write;
// ----------------------------------------------------------------------------

use crate::error::Result;
use crate::error;

use snafu::{ResultExt, OptionExt, ensure};
use log::*;

use std::process::Command;


pub fn untar(path: &std::path::PathBuf) -> Result<std::path::PathBuf> {
    ensure!(path.is_file(), error::FileDoesNotExist{path: path.clone()});

    // Retrieve the dirname and basename of the archive. Both of them MUST
    // be valid paths, otherwise we messed up earlier in the execution of
    // the program.
    let dir = path.as_path().parent()
        .context(error::IllFormedPath{path: path.clone()})?;

    // Run the tar command. Error will be reported on stderr, because the
    // child inherits stdout/stderr.
    // We then check that tar does not fail before continuing.
    info!("Decompressing {:#?}", path);
    let status = Command::new("tar")
        .arg("-C")
        .arg(dir)
        .arg("-xf")
        .arg(path)
        .status()
        .context(error::ProgFailed{ proc: "tar".to_string() })?;
    ensure!(status.success(), error::TarFailed{ path: path.clone() });

    // If the archive is in 'download/X.tar.xz', the output path MUST be
    // 'download/X', because this is what u-boot and linux do, and we
    // rely on that behavior.
    let mut p = path.clone();
    p.set_extension(""); /* Strip .xz */
    p.set_extension(""); /* Strip .tar */
    ensure!(p.is_dir(), error::UnexpectedUntar{arch: path.clone(), dir: p.clone()});
    Ok(p)
}

pub fn xz(path: &std::path::PathBuf) -> Result<std::path::PathBuf> {
    let xz_file = std::fs::File::open(path)
        .context(error::FailedToOpen{path: path.clone()})?;
    let mut decoder = xz2::read::XzDecoder::new(xz_file);
    let mut data = String::new();
    decoder.read_to_string(&mut data)
        .context(error::FailedToDecodeXz{path: path.clone()})?;

    // Compose the path to the decompressed file. That's just the .xz file
    // stripped from its extension.
    let mut file_path = path.clone();
    file_path.set_extension("");
    let mut file = std::fs::File::create(&file_path)
        .context(error::CreateFileError{path: file_path.clone()})?;
    file.write_all(data.as_bytes())
        .context(error::FailedToWrite{path: file_path.clone()})?;
    Ok(file_path)
}
