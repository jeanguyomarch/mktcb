/* This is part of mktcb - which is under the MIT License ********************/

// Traits ---------------------------------------------------------------------
use std::io::Write;
// ----------------------------------------------------------------------------

use std::path::PathBuf;
use std::process::{Command, Stdio};
use url::Url;
use log::*;

use snafu::{ResultExt, ensure};

use crate::error::Result;
use crate::error;
use crate::download;
use crate::decompress;
use crate::toolchain::Toolchain;
use crate::config::Config;
use crate::interrupt::Interrupt;
use crate::patch;
use crate::util;

struct Version {
    maj: usize,
    min: usize,
    mic: usize,
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.maj, self.min, self.mic)
    }
}

pub struct Linux {
    version: Version,
    version_file: PathBuf,
    download_dir: PathBuf,
    source_dir: PathBuf,
    patches_dir: PathBuf,
    build_dir: PathBuf,
    pkg_dir: PathBuf,
    config: Option<PathBuf>,
    base_url: url::Url,
    http_handle: curl::easy::Easy,
    target: String,
    interrupt: Interrupt,
    arch: String,
    name: String,
    jobs: usize,
}

impl Linux {
    /// Retrieve the current Linux version from the version file that resides
    /// on the filesystem. The file must exist and be valid for the operation
    /// to take place
    fn load_version(&mut self) -> Result<()> {
        ensure!(self.version_file.exists(), error::LinuxNotFetched{});
        let data = util::read_file(&self.version_file)?;
        self.version = make_version(&data)?;
        Ok(())
    }

    /// Dump the current Linux version in the version file.
    /// This allows for successive calls to mktcb to keep track of the next
    /// updates of the Linux kernel.
    fn write_version(&self) -> Result<()> {
        let mut file = std::fs::File::create(&self.version_file).context(
            error::CreateFileError{path: self.version_file.clone()})?;
        write!(file, "{}", self.version)
            .context(error::FailedToWrite{path: self.version_file.clone()})?;
        Ok(())
    }

    /// Depending on whether the micro is 0 or not, the patch file does not
    /// have the same format.
    ///
    /// This function returns the URL pointing to the expected patch file
    /// allowing to bump the version.
    fn get_next_patch_url(&self) -> Result<(url::Url, String)> {
        if self.version.mic == 0 {
            let file = format!("patch-{}.{}.{}.xz",
                self.version.maj, self.version.min, self.version.mic + 1);
            let url = self.base_url.join(&file).context(error::InvalidLinuxURL{})?;
            Ok((url, file))
        } else {
            let file = format!("patch-{}-{}.xz",
                self.version, self.version.mic + 1);
            let url = self.base_url.join("incr/")
                .context(error::InvalidLinuxURL{})
                .and_then(|u| {
                    u.join(&file).context(error::InvalidLinuxURL{})
                })?;
            Ok((url, file))
        }
    }

    /// Download the whole source tree of the Linux kernel. They will
    /// end up decompressed in the download directory, and the version
    /// file will be initialized to the first release.
    fn download_archive(&mut self) -> Result<()> {
        // Determine the name of the linux archive to be downloaded.
        // Since the Linux maintainers are decent people, the downloaded
        // file will have the exact same name.
        let arch = format!("linux-{}.{}.tar.xz",
            self.version.maj, self.version.min);

        // Compose the URL to be queried for the Linux archive.
        let url = self.base_url.join(&arch).context(error::InvalidLinuxURL{})?;

        // Download and unpack the sources
        download::to_unpacked_dir(
            &mut self.http_handle, &url, &self.download_dir, &self.source_dir)?;

        // We now have the full source tree. They MAY be patched. If a signal
        // happens between patching and writing the version, the whole source
        // tree will get corrupted (we cannot possibly know, without great manual
        // effort in which state it was left).
        // So, prevent SIGINT to destroy the directory.
        self.interrupt.lock();
        self.reconfigure()?;
        // We have just downloaded the sources. Apply patches, if any.
        self.apply_patches()?;
        // Finally, store the version
        self.write_version()
    }

    /// Go over the patches for a given version of Linux, if they exist, and
    /// apply them to the source tree.
    /// NOTE: this function is called when the lock for patches is taken.
    /// Don't lock!!
    fn apply_patches(&self) -> Result<()> {
        let mut try_path = self.patches_dir.clone();
        try_path.push(if self.version.mic == 0 {
            format!("{}.{}", self.version.maj, self.version.min)
        } else {
            format!("{}", self.version)
        });

        patch::apply_patches_in(&try_path, &self.source_dir)
    }


    /// Generate the command to call make in Linux' sources
    fn get_make_cmd(&self, toolchain: &Toolchain) -> Command {
        let mut make_cmd = Command::new("make");
        make_cmd
            .arg("-C").arg(self.source_dir.clone())
            .arg(format!("-j{}", self.jobs))
            .arg(format!("O={}", self.build_dir.to_str().unwrap()))
            .arg(format!("ARCH={}", self.arch))
            .arg(format!("CROSS_COMPILE={}", toolchain.cross_compile));
        make_cmd
    }

    pub fn fetch(&mut self) -> Result<()> {
        if ! self.version_file.exists() {
            ensure!(! self.source_dir.exists(), error::CorruptedSourceDir{
                dir: self.source_dir.clone(),
                version_file: self.version_file.clone(),
            });
            info!("File {:#?} not found. Downloading Linux archive...", self.version_file);
            self.download_archive()?;
        } else {
            self.load_version()?;
        }

        // And now, we will apply all patches that were released since the
        // last checkout.
        loop {
            let (url, file) = self.get_next_patch_url()?;
            if download::check(&mut self.http_handle, &url)? {
                // There is a patch available!
                info!("Upgrading from version {}", self.version);

                // Download the file. It is a compressed diff file (.xz)
                let mut path = self.download_dir.clone();
                path.push(file);
                download::to_file(&mut self.http_handle, &url, &path)?;

                // Decompress the downloaded file to get the actual diff.
                let diff_file = decompress::xz(&path)?;
                {
                    // From this point, we will modify the sources. So make
                    // sure that interruptions will not leave the source tree
                    // in a corrupted state.
                    self.interrupt.lock();
                    patch::patch(&self.source_dir, &diff_file)?;

                    // We have upgraded to a new version of the Linux kernel.
                    // Apply the patches fo this revision, if any. Then, update the
                    // version file.
                    self.version.mic += 1;
                    self.apply_patches()?;
                    self.write_version()?;
                }
            } else {
                info!("Last version: {}", self.version);
                break;
            }
        }

        Ok(())
    }

    /// Create a copy of the configuration described by the target (if any)
    pub fn reconfigure(&self) -> Result<()> {
        // Copy the configuration to the build dir, if any.
        util::copy_config(&self.config, &self.build_dir)
    }

    /// Take the Linux Kconfig from the build directory and use it as the main
    /// configuration file
    pub fn save_config(&self) -> Result<()> {
        ensure!(self.config.is_some(), error::NoLinux{});
        util::save_config(&self.config.as_ref().unwrap(), &self.build_dir)
    }

    /// Check if a new update patch is present. If not, there are no updates.
    /// If we cannot find the version file, we *assume* the sources were not
    /// retrieved, so they technically can be updated (going from nothing to
    /// something).
    pub fn check_update(&mut self) -> Result<bool> {
        if self.version_file.exists() {
            self.load_version()?;
            let (url, _) = self.get_next_patch_url()?;
            download::check(&mut self.http_handle, &url)
        } else {
            Ok(true)
        }
    }

    pub fn make(&mut self, make_target: &str, toolchain: &Toolchain) -> Result<()> {
        toolchain.fetch()?;
        self.load_version()?;
        let status = self.get_make_cmd(toolchain)
            .arg("--")
            .arg(make_target)
            .status()
            .context(error::ProgFailed{ proc: "make".to_string() })?;
        ensure!(status.success(), error::MakeFailed{
            target: make_target.to_string() });
        Ok(())
    }
}


/// Create the version structure from a textual input. The source of the
/// input can be either from the TOML configuration (X.Y) or from the
/// version file (X.Y.Z).
fn make_version(str_version: &str) -> Result<Version> {
    fn parse_v(number: &str) -> Result<usize> {
        number.parse().context(error::InvalidVersionNumber{
            string: number.to_string(),
        })
    }

    let vec: Vec<&str> = str_version.split('.').collect();
    ensure!(vec.len() == 2 || vec.len() == 3, error::InvalidVersionFormat{
        orig: str_version.to_string()
    });

    Ok(Version {
        maj: parse_v(vec[0])?,
        min: parse_v(vec[1])?,
        mic: if vec.len() == 3 {
            parse_v(vec[2])?
        } else {
            0
        },
    })
}

/// Compose a path involving a given Linux version
fn make_version_dir(base_dir: &PathBuf, version: &Version) -> PathBuf {
    let mut path = base_dir.clone();
    path.push(format!("linux-{}.{}", version.maj, version.min));
    path
}

/// Compose the build directory in which the Linux kernel will be built
fn make_build_dir(base_dir: &PathBuf, version: &Version, target: &str) -> PathBuf {
    let mut path = base_dir.clone();
    path.push(format!("linux-{}.{}-{}", version.maj, version.min, target));
    path
}

fn make_patches_dir(base_dir: &PathBuf) -> PathBuf {
    let mut path = base_dir.clone();
    path.push("patches");
    path.push("linux");
    path
}


/// Create a new instance for Linux management
pub fn new(config: &Config, interrupt: Interrupt) -> Result<Linux> {
    let linux = config.linux.as_ref().unwrap(); // Already checked
    let version = make_version(&linux.version)?;
    let mut v_file = config.download_dir.clone();
    v_file.push(format!("linux-{}.{}.version", version.maj, version.min));

    let mut pkg_dir = config.build_dir.clone();
    pkg_dir.push("packages");

    let url = format!("https://cdn.kernel.org/pub/linux/kernel/v{}.x/",
        version.maj);
    Ok(Linux {
        download_dir: config.download_dir.clone(),
        source_dir: make_version_dir(&config.download_dir, &version),
        build_dir: make_build_dir(&config.build_dir, &version, &config.target),
        pkg_dir: pkg_dir,
        patches_dir: make_patches_dir(&config.lib_dir),
        config: linux.config.clone(),
        version: version,
        version_file: v_file,
        base_url: Url::parse(&url).context(error::InvalidLinuxURL{})?,
        http_handle: curl::easy::Easy::new(),
        jobs: config.jobs,
        arch: config.toolchain.linux_arch.clone(),
        target: config.target.clone(),
        name: config.target_name.clone(),
        interrupt: interrupt,
    })
}
