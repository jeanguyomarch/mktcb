/* This is part of mktcb - which is under the MIT License ********************/

use std::path::PathBuf;
use snafu::{ResultExt, ensure};
use clap::ArgMatches;
use serde_derive::Deserialize;
use serde::de;
use log::*;

use crate::error::Result;
use crate::error;

#[derive(Debug)]
pub struct Config {
    pub build_dir: PathBuf,
    pub lib_dir: PathBuf,
    pub download_dir: PathBuf,
    pub toolchain: ToolchainConfig,
    pub linux: ComponentConfig,
    pub uboot: ComponentConfig,
    /// Pretty name of the target
    pub target_name: String,
    /// Stem of the target
    pub target: String,
    pub jobs: usize,
}

#[derive(Debug, Deserialize)]
pub struct ToolchainConfig {
    pub url: String,
    pub linux_arch: String,
    pub uboot_arch: String,
    pub debian_arch: String,
    pub cross_compile: String,
}

#[derive(Debug, Deserialize)]
pub struct ComponentConfig {
    pub version: String,
    pub config: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct TargetConfig {
    toolchain: String,
    name: String,
    linux: ComponentConfig,
    uboot: ComponentConfig,
}


/// Read the contents of a input file. In case of failure, the error is wrapped
/// to my pretty error that clearly states why it failed.
fn load_file(path: &PathBuf) -> Result<Vec<u8>> {
    std::fs::read(&path).context(error::FailedToRead {
        path: path.clone(),
    })
}

/// Generic helper that loads the contents of a TOML file into de-serialized
/// rust structures
fn load_toml<'de, T>(file_contents: &'de Vec<u8>, path: &PathBuf) -> Result<T>
where
    T: de::Deserialize<'de>
{
    let decoded: T = toml::from_slice(file_contents.as_slice())
        .context(error::FailedToDeser{ path: path.clone()})?;
    Ok(decoded)
}

/// Once we have loaded a target configuration, the config paths must be
/// updated to reflect their actual location. This function does exactly
/// this, and makes sure the file is a valid one
fn make_config_path(library: &PathBuf, comp: &str, item: &ComponentConfig) -> Result<Option<PathBuf>> {
    if let Some(cfg) = &item.config {
        let mut path = library.clone();
        path.push("configs");
        path.push(comp);
        path.push(item.version.clone());
        path.push(cfg);

        ensure!(path.exists(), error::FileDoesNotExist{ path: path.clone() });
        Ok(Some(path))
    } else {
        Ok(None)
    }
}

/// Load the contents of the TOML file that describes the target as a
/// rust object. It also performs in-place modification to normalize
/// paths.
fn load_target_config(library: &PathBuf, target: &str) -> Result<TargetConfig> {
    let mut path = library.clone();
    path.push("targets");
    path.push(target);
    path.set_extension("toml");

    let file_contents = load_file(&path)?;
    let mut cfg = load_toml::<TargetConfig>(&file_contents, &path)?;

    info!("Using target configuration at path {:#?}", path);

    cfg.linux.config = make_config_path(library, "linux", &cfg.linux)?;
    cfg.uboot.config = make_config_path(library, "uboot", &cfg.uboot)?;

    Ok(cfg)
}

fn load_toolchain_config(library: &PathBuf, toolchain: &str) -> Result<ToolchainConfig> {
    let mut path = library.clone();
    path.push("toolchains");
    path.push(toolchain);
    path.set_extension("toml");

    let file_contents = load_file(&path)?;
    load_toml::<ToolchainConfig>(&file_contents, &path)
}

pub fn new(matches: &ArgMatches) -> Result<Config> {
    let current_dir = std::env::current_dir().context(error::CwdAccess{})?;

    // Library - if not provided by the user, default to the current
    // working directory
    let library = match matches.value_of("library") {
        Some(val) => {
            PathBuf::from(val)
                .canonicalize()
                .context(error::CanonFailed{dir: val.clone()})?
        },
        None => current_dir.clone(),
    };

    // Build directory - if not provided by the user, default to a
    // directory named build/ in the current working directory
    let download_dir = match matches.value_of("download_dir") {
        Some(val) => {
            std::fs::create_dir_all(val).context(
                error::CreateDirError{ path: val.clone() })?;
            PathBuf::from(val).canonicalize()
                .context(error::CanonFailed{dir: val.clone()})?
        },
        None => {
            let mut download_dir = current_dir.clone();
            download_dir.push("download");
            download_dir
        }
    };

    // Download directory - if not provided by the user, default to a
    // directory named download/ in the current working directory
    let build_dir = match matches.value_of("build_dir") {
        Some(val) => {
            std::fs::create_dir_all(val).context(
                error::CreateDirError{ path: val.clone() })?;
            PathBuf::from(val).canonicalize()
                .context(error::CanonFailed{dir: val.clone()})?
        },
        None => {
            let mut build_dir = current_dir;
            build_dir.push("build");
            build_dir
        }
    };

    // Target  - it is required, so it is guaranteed to have a value
    let target = matches.value_of("target").unwrap();

    // Jobs - make sure the value provided by the user is valid. It no
    // value was provided, use the number of CPUs + 2
    let jobs = match matches.value_of("jobs") {
        Some(val) => {
            let nb = val.parse().context(error::InvalidJobNumber{})?;
            ensure!(nb > 0, error::ZeroJob{});
            nb
        },
        None => num_cpus::get() + 2,
    };


    // ------------------------------------------------------------------------
    // Load the target TOML file
    let target_cfg = load_target_config(&library, &target)?;


    Ok(Config {
        build_dir: build_dir,
        download_dir: download_dir,
        toolchain: load_toolchain_config(&library, target_cfg.toolchain.as_str())?,
        linux: target_cfg.linux,
        uboot: target_cfg.uboot,
        jobs: jobs,
        target: target.to_string(),
        target_name: target_cfg.name.clone(),
        lib_dir: library,
    })
}
