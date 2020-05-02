/* This is part of mktcb - which is under the MIT License ********************/

use log::{Record, Level, Metadata, LevelFilter};
use snafu::{OptionExt};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
use crate::error::Result;
use crate::error;
use std::io::Write;

struct Logger;

static LOGGER: Logger = Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let mut spec = ColorSpec::new();
            let (lvl, use_stderr) = match record.level() {
                Level::Error => {
                    spec.set_fg(Some(Color::Red));
                    ("error", true)
                },
                Level::Warn => {
                    spec.set_fg(Some(Color::Yellow));
                    ("warning", true)
                },
                Level::Info => {
                    spec.set_fg(Some(Color::Green));
                    ("info", true)
                },
                Level::Debug => {
                    spec.set_fg(Some(Color::Blue));
                    ("debug", true)
                },
                Level::Trace => {
                    spec.set_fg(Some(Color::White));
                    ("trace", true)
                },
            };
            spec.set_intense(true).set_bold(true);
            let (mut stream, use_color) = if use_stderr {
                (StandardStream::stdout(ColorChoice::Auto),
                 atty::is(atty::Stream::Stdout))
            } else {
                (StandardStream::stderr(ColorChoice::Auto),
                 atty::is(atty::Stream::Stderr))
            };

            // Actually log... We purely ignore errors if we fail to set the
            // color and other tty attributes, because logging is more important
            // than fancyness. Also, if we fail to actually log, we try using
            // eprintln!(), hoping for the best... If log fails anyway, we are
            // likely screwed.
            if use_color {
                let _ = stream.set_color(&spec);
            }
            if let Err(_) = write!(&mut stream, "{}", lvl) {
                eprintln!("{}", lvl);
            }
            if use_color {
                spec.clear();
                let _ = stream.set_color(&spec);
            }
            if let Err(_) = writeln!(&mut stream, ": {}", record.args()) {
                eprintln!(": {}", record.args());
            }
        }
    }

    fn flush(&self) {}
}

pub fn init(max_level: LevelFilter) -> Result<()> {
    log::set_logger(&LOGGER).map(|()| {
        log::set_max_level(max_level)
    }).ok().context(error::LogInitFailed{})

    //let log = &mut LOGGER;
    //LOG

    //log.stdout_use_colors = atty::is(atty::Stream::Stdout);
    //log.stderr_use_colors = atty::is(atty::Stream::Stderr);


}

