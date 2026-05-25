//*********************************************
// Authors:
// Francesco Boccola (francesco.boccola@unina.it)
//*********************************************
//
// Xen tick source.
//
// Under Xen Dom0 the raw counter MMIO is not accessible from
// userspace, so the architectural counter is exposed by a companion
// kernel module as a character device:
//
//     $ cat /dev/arm_timer
//     43332016151
//
// Each read returns the current 64-bit value of CNTPCT_EL0 as a
// decimal ASCII string terminated by '\n'. The tick frequency is
// the same arch_timer as on the Jailhouse backend.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::sync::Mutex;

use logging::timer::TickSource;

const TIMER_DEV: &str = "/dev/arm_timer";

pub struct XenTickSource {
    // The char device is kept open for the process lifetime to avoid
    // a per-read open/close. Some kernel-module implementations don't
    // support seek; we reopen as a fallback if seek-to-zero fails.
    file: Mutex<Option<File>>,
}

impl XenTickSource {
    pub fn new() -> std::io::Result<Self> {
        let file = OpenOptions::new().read(true).open(TIMER_DEV)?;
        Ok(Self {
            file: Mutex::new(Some(file)),
        })
    }
}

impl TickSource for XenTickSource {
    fn read_ticks(&self) -> u64 {
        let mut guard = self.file.lock().unwrap();

        if guard.is_none() {
            match OpenOptions::new().read(true).open(TIMER_DEV) {
                Ok(f) => *guard = Some(f),
                Err(_) => return 0,
            }
        }

        if guard.as_mut().unwrap().seek(SeekFrom::Start(0)).is_err() {
            match OpenOptions::new().read(true).open(TIMER_DEV) {
                Ok(f) => *guard = Some(f),
                Err(_) => return 0,
            }
        }

        let file = guard.as_mut().unwrap();
        let mut buf = [0u8; 32];
        let n = match file.read(&mut buf) {
            Ok(n) => n,
            Err(_) => return 0,
        };
        if n == 0 {
            return 0;
        }

        let s = match std::str::from_utf8(&buf[..n]) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        s.trim().parse::<u64>().unwrap_or(0)
    }
}

// Convenience: build the source and install it into the logging crate.
// Called once at program start from main.
pub fn install() -> std::io::Result<()> {
    let src = XenTickSource::new()?;
    logging::timer::initialize_with(Box::new(src))
}
