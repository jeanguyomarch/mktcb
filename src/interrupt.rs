/* This is part of mktcb - which is under the MIT License ********************/

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::error::Result;
use crate::error;
use snafu::{ResultExt};
use log::*;

pub struct Interrupt {
    must_stop: Arc<AtomicBool>,
    locked: Arc<AtomicBool>,
}

pub struct Guard {
    must_stop: Arc<AtomicBool>,
    locked: Arc<AtomicBool>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        if self.must_stop.load(Ordering::SeqCst) {
            debug!("An interrupt request will now be serviced");
            std::process::exit(-1);
        }
        self.locked.store(false, Ordering::SeqCst);
    }
}

impl Interrupt {

    pub fn lock(&self) -> Guard {
        assert!(! self.locked.load(Ordering::SeqCst),
            "Recursive lock detected. This is forbidden.");

        self.locked.store(true, Ordering::SeqCst);
        Guard {
            must_stop: self.must_stop.clone(),
            locked: self.locked.clone(),
        }
    }

}

/// Retrieve the global instance of interrupt handlers.
///
/// It allows to catch the "CTRL-C" to perform a proper, non-corrupting exit.
pub fn get() -> Result<Interrupt> {
    let interrupt = Interrupt {
        must_stop: Arc::new(AtomicBool::new(false)),
        locked: Arc::new(AtomicBool::new(false)),
    };

    let must_stop = interrupt.must_stop.clone();
    let locked = interrupt.locked.clone();

    ctrlc::set_handler(move || {
        error!("interruption requested by user!");

        // The user wanted to interrupt the program. Fine, but let's not
        // corrupt the sources. If we are paching them (i.e. we called
        // lock()), don't do anything right now, wait for the lock to
        // expire. Otherwise, die right now.
        if locked.load(Ordering::SeqCst) {
            must_stop.store(true, Ordering::SeqCst);
        } else {
            std::process::exit(-1);
        }
    }).context(error::CtrlCFailed{})?;

    Ok(interrupt)
}

