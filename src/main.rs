/* This is part of mktcb - which is under the MIT License ********************/

mod config;
mod decompress;
mod download;
mod error;
mod interrupt;
mod linux;
mod logging;
mod patch;
mod toolchain;
mod uboot;
mod util;

// Traits ---------------------------------------------------------------------
use std::io::Write;
// ----------------------------------------------------------------------------

use snafu::{ResultExt, ensure};
use clap::{Arg, App, SubCommand};
use crate::error::Result;
use log::*;

use std::path::PathBuf;


fn run(matches: &clap::ArgMatches) -> Result<()> {
    let config = config::new(&matches)?;
    let interrupt = interrupt::get()?;

    if let Some(matches) = matches.subcommand_matches("linux") {
        ensure!(config.linux.is_some(), error::NoLinux{});
        let mut agent = linux::new(&config, interrupt)?;

        if matches.is_present("check-update") {
            if agent.check_update()? {
                info!("A new version of the Linux kernel is available");
            } else {
                std::process::exit(100);
            }
        }
        if matches.is_present("fetch") {
            agent.fetch()?;
        }
        if matches.is_present("save-config") {
            agent.save_config()?;
        }
        if matches.is_present("reconfigure") {
            agent.reconfigure()?;
        }
        if matches.occurrences_of("make") != 0 {
            // Retrive the make target to be run. It is a required argument,
            // so we can safely unwrap().
            let target = matches.value_of("make").unwrap();

            let toolchain = toolchain::new(&config)?;
            agent.make(target, &toolchain)?;
        }
    } else if let Some(matches) = matches.subcommand_matches("uboot") {
        ensure!(config.uboot.is_some(), error::NoLinux{});
        let agent = uboot::new(&config, interrupt)?;
        if matches.is_present("fetch") {
            agent.fetch()?;
        }
        if matches.is_present("save-config") {
            agent.save_config()?;
        }
        if matches.is_present("reconfigure") {
            agent.reconfigure()?;
        }
        if matches.occurrences_of("make") != 0 {
            // Retrive the make target to be run. It is a required argument,
            // so we can safely unwrap().
            let target = matches.value_of("make").unwrap();

            let toolchain = toolchain::new(&config)?;
            agent.make(target, &toolchain)?;
        }
    }
    Ok(())
}

fn main() {
    let matches = App::new("mktcb")
        .version("0.1.0")
        .author("Jean Guyomarc'h <jean@guyomarch.bzh>")
        .about("Build the Trusted Computing Base (TCB)")
        .arg(Arg::with_name("library")
            .short("L")
            .long("library")
            .value_name("DIR")
            .help("Set the path to the TCB library")
            .takes_value(true))
        .arg(Arg::with_name("build_dir")
            .short("B")
            .long("build-dir")
            .value_name("DIR")
            .help("Set the path to the build directory")
            .takes_value(true))
        .arg(Arg::with_name("download_dir")
            .short("D")
            .long("download-dir")
            .value_name("DIR")
            .help("Set the path to the download directory")
            .takes_value(true))
        .arg(Arg::with_name("target")
            .short("t")
            .long("target")
            .value_name("TARGET")
            .required(true)
            .help("Name of the target to operate on")
            .takes_value(true))
        .arg(Arg::with_name("jobs")
            .short("j")
            .long("jobs")
            .value_name("JOBS")
            .help("Set the number of parallel jobs to be used")
            .takes_value(true))
        .subcommand(SubCommand::with_name("linux")
            .about("operations on the Linux kernel")
            .arg(Arg::with_name("make")
                .long("make")
                .value_name("TARGET")
                .default_value("all")
                .help("Run a make target in the Linux tree")
                .takes_value(true))
            .arg(Arg::with_name("check-update")
                .long("check-update")
                .help("Check whether a new update is available on kernel.org. \
                    If no update is available, mkctb will exit with status 100."))
            .arg(Arg::with_name("reconfigure")
                .long("reconfigure")
                .help("Re-generate the Linux .config from the target config"))
            .arg(Arg::with_name("save-config")
                .long("save-config")
                .help("Use the Linux .config as the new TCB config (overwrites)"))
            .arg(Arg::with_name("fetch")
                .long("fetch")
                .help("Retrieve the latest version of the Linux kernel")))
        .subcommand(SubCommand::with_name("uboot")
            .about("operations on the U-Boot")
            .arg(Arg::with_name("make")
                .long("make")
                .value_name("TARGET")
                .default_value("all")
                .help("Run a make target in the U-Boot tree")
                .takes_value(true))
            .arg(Arg::with_name("reconfigure")
                .long("reconfigure")
                .help("Re-generate the U-Boot .config from the target config"))
            .arg(Arg::with_name("save-config")
                .long("save-config")
                .help("Use the U-Boot .config as the new TCB config (overwrites)"))
            .arg(Arg::with_name("fetch")
                .long("fetch")
                .help("Retrieve U-Boot")))
        .get_matches();

    if let Err(err) = logging::init(log::LevelFilter::Trace) {
        eprintln!("ERROR: {}", err);
        std::process::exit(3);
    };

    match run(&matches) {
        Ok(()) => {},
        Err(err) => {
            error!("{}", err);
            std::process::exit(2);
        }
    }
}
