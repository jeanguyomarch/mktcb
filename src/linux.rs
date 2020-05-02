/* This is part of mktcb - which is under the MIT License ********************/

use std::io::Write;

use crate::config::Config;
use std::path::PathBuf;
use url::Url;
use log::*;

use snafu::{ResultExt, ensure};
use crate::error::Result;
use crate::error;
use crate::download;
use crate::decompress;
use crate::interrupt::Interrupt;
use crate::patch;

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
    build_dir: PathBuf,
    base_url: Url,
    http_handle: curl::easy::Easy,
    interrupt: Interrupt,
}

impl Linux {
    /// Retrieve the current Linux version from the version file that resides
    /// on the filesystem. The file must exist and be valid for the operation
    /// to take place
    fn load_version(&mut self) -> Result<()> {
        ensure!(self.version_file.exists(), error::LinuxNotFetched{});
        let contents = std::fs::read(&self.version_file).context(
            error::FailedToReadVersion { path: format!("{:#?}", self.version_file)
        })?;
        let data = std::str::from_utf8(contents.as_slice()).context(
            error::FailedToDecodeUTF8{}
        )?;
        self.version = make_version(data)?;
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
        // First, make sure that the download directory exists. Create
        // it if this is not the case.
        std::fs::create_dir_all(&self.download_dir).context(
            error::CreateDirError{ path: self.download_dir.clone() })?;

        // Determine the name of the linux archive to be downloaded.
        // Since the Linux maintainers are decent people, the downloaded
        // file will have the exact same name.
        let arch = format!("linux-{}.{}.tar.xz",
            self.version.maj, self.version.min);

        // Compose the URL to be queried for the Linux archive.
        let url = self.base_url.join(&arch).context(error::InvalidLinuxURL{})?;

        // Create the file that will hold the contents of the Linux
        // archive.
        let mut tar_path = self.download_dir.clone();
        tar_path.push(arch);

        // Retrieve the .tar.xz archive
        download::to_file(&mut self.http_handle, &url, &tar_path)?;

        // Uncompress it!
        let out_dir = decompress::untar(&tar_path)?;
        ensure!(out_dir == self.source_dir, error::UnexpectedUntar{
            arch: tar_path.clone(), dir: self.source_dir.clone()});

        // We now have the full source tree. They MAY be patched. If a signal
        // happens between patching and writing the version, the whole source
        // tree will get corrupted (we cannot possibly know, without great manual
        // effort in which state it was left).
        // So, prevent SIGINT to destroy the directory.
        {
            self.interrupt.lock();
            // We have just downloaded the sources. Apply patches, if any.
            self.apply_patches()?;
            // Finally, store the version
            self.write_version()
        }
    }

    /// TODO
    fn apply_patches(&self) -> Result<()> {
        Ok(())
    }

    pub fn fetch(&mut self) -> Result<()> {
        if ! self.version_file.exists() {
            ensure!(! self.source_dir.exists(), error::CorruptedSourceDir{
                linux_dir: self.source_dir.clone(),
                version_file: self.version_file.clone(),
            });
            info!("{:#?} not found. Downloading Linux archive...", self.version_file);
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

    /// Check if a new update patch is present. If not, there are no updates.
    pub fn check_update(&mut self) -> Result<bool> {
        if self.version_file.exists() {
            self.load_version()?;
            let (url, _) = self.get_next_patch_url()?;
            download::check(&mut self.http_handle, &url)
        } else {
            Ok(true)
        }
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
fn make_dir_path(base_dir: &PathBuf, version: &Version) -> PathBuf {
    let mut path = base_dir.clone();
    path.push(format!("linux-{}.{}", version.maj, version.min));
    path
}

/// Create a new instance for Linux management
pub fn new(config: &Config, interrupt: Interrupt) -> Result<Linux> {
    let version = make_version(&config.linux.version)?;
    let mut v_file = config.download_dir.clone();
    v_file.push(format!("linux-{}.{}.version", version.maj, version.min));

    let url = format!("https://cdn.kernel.org/pub/linux/kernel/v{}.x/",
        version.maj);
    Ok(Linux {
        download_dir: config.download_dir.clone(),
        source_dir: make_dir_path(&config.download_dir, &version),
        build_dir: make_dir_path(&config.build_dir, &version),
        version: version,
        version_file: v_file,
        base_url: Url::parse(&url).context(error::InvalidLinuxURL{})?,
        http_handle: curl::easy::Easy::new(),
        interrupt: interrupt,
    })
}
