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
    debian_arch: String,
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

    /// Retrieve the path to the debian package containing the
    /// linux-image.
    /// Upon success, the file is guaranteed to be valid.
    fn get_linux_image_deb_pkg(&self) -> Result<PathBuf> {
        // Debian packages (images) have the following form:
        //     ../linux-image-5.4.38_1_armhf.deb
        // relative to the linux build directory.
        // 5.4.38 is obviously the version, and 1 is the debian revision, that
        // is enforced via a make variable
        let base = format!("linux-image-{}_1_{}.deb",
            self.version, self.debian_arch);
        let mut path = self.build_dir.clone();
        path.pop();
        path.push(base);

        // Check that the debian package exist before returning its path
        ensure!(path.is_file(), error::NoPackage{ path: path.clone()});
        Ok(path)
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

    /// Build a Debian meta-package allowing to perform easy upgrades of
    /// the Linux kernel.
    /// Upon success, the path to the created debian package is returned.
    pub fn debpkg(&mut self, toolchain: &Toolchain) -> Result<Vec<PathBuf>> {
        toolchain.fetch()?;
        self.load_version()?;

        let make_target = "bindeb-pkg";
        let status = self.get_make_cmd(toolchain)
            .arg("KDEB_PKGVERSION=1")
            .arg("--")
            .arg(make_target)
            .status()
            .context(error::ProgFailed{ proc: "make".to_string() })?;
        ensure!(status.success(), error::MakeFailed{target: make_target.to_string()});

        let mut package = format!("linux-image-{}.{}-{}",
                self.version.maj, self.version.min, self.target);
        // Compose the path to the DEBIAN directory and create it.
        let mut deb_dir = self.pkg_dir.clone();
        deb_dir.push(&package);
        let mut deb = deb_dir.clone();
        deb.push("DEBIAN");
        std::fs::create_dir_all(&deb).context(
            error::CreateDirError{ path: deb.clone() })?;
        deb.push("control");

        // Create the contents of the DEBIAN/control file. It is automatically
        // generated from the current state of the Linux sources.
        // Note that this is scoped so that the DEBIAN/control file is EFFECTIVELY
        // flushed to the filesystem before dpkg-deb tries to read it.
        {
            let maintainer = util::getenv("MAINTAINER")?;
            let control = format!("
Package: {}
Architecture: {}
Maintainer: {}
Description: Linux kernel, version {}.{}.z for {}
 This is a meta-package allowing to manage updates of the Linux kernel
 for the {}
Depends: linux-image-{}
Version: {}
Section: custom/kernel
Priority: required
",
                package,
                self.debian_arch,
                maintainer,
                self.version.maj, self.version.min, self.name,
                self.name,
                self.version,
                self.version);
            let mut file = std::fs::File::create(&deb)
                .context(error::CreateFileError{path: deb.clone()})?;
            file.write_all(control.as_bytes())
                .context(error::FailedToWrite{
                    path: deb.clone(),
                })?;
        }

        // Run dpkg-deb to create the meta-package
        let status = Command::new("dpkg-deb")
            .arg("--build")
            .arg(&package)
            .current_dir(&self.pkg_dir)
            .stdin(Stdio::null())
            .status()
            .context(error::ProgFailed{ proc: "dpkg-deb".to_string() })?;
        ensure!(status.success(), error::DebFailed{package: package});

        // Finally, return the path to the debian file. Hoping that it
        // was indeed created.
        let mut result = self.pkg_dir.clone();
        package.push_str(".deb");
        result.push(package);
        ensure!(result.is_file(), error::NoPackage{path:result.clone()});

        let image = self.get_linux_image_deb_pkg()?;
        Ok(vec![
            image,
            result,
        ])
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
        build_dir: make_version_dir(&config.build_dir, &version),
        pkg_dir: pkg_dir,
        patches_dir: make_patches_dir(&config.lib_dir),
        config: linux.config.clone(),
        version: version,
        version_file: v_file,
        base_url: Url::parse(&url).context(error::InvalidLinuxURL{})?,
        http_handle: curl::easy::Easy::new(),
        jobs: config.jobs,
        arch: config.toolchain.linux_arch.clone(),
        debian_arch: config.toolchain.debian_arch.clone(),
        target: config.target.clone(),
        name: config.target_name.clone(),
        interrupt: interrupt,
    })
}
