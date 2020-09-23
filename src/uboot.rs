/* This is part of mktcb - which is under the MIT License ********************/

// Traits ---------------------------------------------------------------------
use std::io::Write;
// ----------------------------------------------------------------------------

use std::path::PathBuf;
use std::process::Command;

use snafu::{ResultExt, ensure};

use crate::error::Result;
use crate::error;
use crate::config::Config;
use crate::download;
use crate::patch;
use crate::util;
use crate::toolchain::Toolchain;
use crate::interrupt::Interrupt;

pub struct Uboot {
    download_dir: PathBuf,
    source_dir: PathBuf,
    build_dir: PathBuf,
    patches_dir: PathBuf,
    version: String,
    version_file: PathBuf,
    config: Option<PathBuf>,
    url: url::Url,
    interrupt: Interrupt,
    arch: String,
    jobs: usize,
}

impl Uboot {
    fn write_version(&self) -> Result<()> {
        let mut file = std::fs::File::create(&self.version_file).context(
            error::CreateFileError{path: self.version_file.clone()})?;
        write!(file, "{}", self.version)
            .context(error::FailedToWrite{path: self.version_file.clone()})?;
        Ok(())
    }

    fn download(&self) -> Result<()> {
        let mut http_handle = curl::easy::Easy::new();
        download::to_unpacked_dir(
            &mut http_handle, &self.url, &self.download_dir, &self.source_dir)?;

        // Copy the initial configuration, if any
        util::copy_config(&self.config, &self.build_dir)?;

        // Apply patches on the working directory and then write the version.
        // A sigint may not interrupt this...
        self.interrupt.lock();
        patch::apply_patches_in(&self.patches_dir, &self.source_dir)?;
        self.write_version()
    }

    pub fn make(&self, make_target: &str, toolchain: &Toolchain) -> Result<()> {
        toolchain.fetch()?;
        let status = Command::new("make")
            .arg(format!("O={}", self.build_dir.to_str().unwrap()))
            .arg(format!("ARCH={}", self.arch))
            .arg(format!("CROSS_COMPILE={}", toolchain.cross_compile))
            .arg("-C").arg(self.source_dir.clone())
            .arg(format!("-j{}", self.jobs))
            .arg("--")
            .arg(make_target)
            .status()
            .context(error::ProgFailed{ proc: "make".to_string() })?;
        ensure!(status.success(), error::MakeFailed{
            target: make_target.to_string() });
        Ok(())
    }

    pub fn fetch(&self) -> Result<()> {
        if ! self.version_file.exists() {
            ensure!(! self.source_dir.exists(), error::CorruptedSourceDir{
                dir: self.source_dir.clone(),
                version_file: self.version_file.clone(),
            });
            self.download()
        } else {
            Ok(())
        }
    }
}

/// Compose a path involving a given U-Boot version
fn make_version_dir(base_dir: &PathBuf, version: &str) -> PathBuf {
    let mut path = base_dir.clone();
    path.push(format!("u-boot-{}", version));
    path
}

fn make_patches_dir(base_dir: &PathBuf, version: &str) -> PathBuf {
    let mut path = base_dir.clone();
    path.push("patches");
    path.push("uboot");
    path.push(version);
    path
}

pub fn new(config: &Config, interrupt: Interrupt) -> Result<Uboot> {
    let uboot = config.uboot.as_ref().unwrap(); // Already checked
    let version = uboot.version.clone();
    let url =  format!("ftp://ftp.denx.de/pub/u-boot/u-boot-{}.tar.bz2", version);

    // Compose the path to the version file
    let mut v_file = config.download_dir.clone();
    v_file.push(format!("u-boot-{}.version", version));

    Ok(Uboot {
        download_dir: config.download_dir.clone(),
        source_dir: make_version_dir(&config.download_dir, &version),
        build_dir: make_version_dir(&config.build_dir, &version),
        patches_dir: make_patches_dir(&config.lib_dir, &version),
        version_file: v_file,
        url: url::Url::parse(&url).context(error::InvalidUbootURL{})?,
        config: uboot.config.clone(),
        version: version,
        arch: config.toolchain.uboot_arch.clone(),
        interrupt: interrupt,
        jobs: config.jobs,
    })
}
