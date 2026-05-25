//*********************************************
// Authors:
// Francesco Boccola (francesco.boccola@unina.it)
//*********************************************
//
// Xen backend timer.
//
// Unlike the Jailhouse backend (which mmaps the physical ARM timer
// counter through /dev/mem), under Xen Dom0 access to the raw
// counter MMIO is blocked. Instead, a companion kernel module
// exposes the architectural counter through a character device:
//
//     $ cat /dev/arm_timer
//     43332016151
//
// Each read returns the current 64-bit value of CNTPCT_EL0
// (or equivalent) as a decimal ASCII string terminated by '\n'.
// Tick frequency is the same arch_timer (~99.99 MHz on the target board).

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::sync::Mutex;

use lazy_static::lazy_static;

const TIMER_DEV: &str = "/dev/arm_timer";
const LOG_PATH: &str = "/usr/share/runPHI/log.txt"; //Change to your desired log file path

lazy_static! {
    static ref TIMER_FILE: Mutex<Option<File>> = Mutex::new(None);
}

fn open_timer() -> io::Result<File> {
    OpenOptions::new().read(true).open(TIMER_DEV)
}

/// Explicitly initialize the timer at program start (optional but recommended)
pub fn initialize() -> io::Result<()> {
    let file = open_timer()?;
    *TIMER_FILE.lock().unwrap() = Some(file);
    Ok(())
}

fn read_ticks() -> u64 {
    let mut guard = TIMER_FILE.lock().unwrap();
    if guard.is_none() {
        match open_timer() {
            Ok(f) => *guard = Some(f),
            Err(_) => return 0,
        }
    }

    let file = guard.as_mut().unwrap();
    if file.seek(SeekFrom::Start(0)).is_err() {
        // Some char devices don't support seeking; fall back to reopening.
        match open_timer() {
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

/// Log timestamp with message
#[inline(never)]
pub fn log_phase(message: &str) -> io::Result<()> {
    let timestamp = read_ticks();

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)?;

    writeln!(file, "{} - {}", timestamp, message)?;

    Ok(())
}

/// Log a pre-captured timestamp with message
#[inline(never)]
pub fn log_phase_at(timestamp: u64, message: &str) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)?;

    writeln!(file, "{} - {}", timestamp, message)?;

    Ok(())
}

/// Ultra-fast timestamp capture without log file I/O.
/// Still costs one syscall (read on /dev/arm_timer) plus a parse.
#[inline(always)]
pub fn capture() -> u64 {
    read_ticks()
}

/// Batch write multiple timestamps
pub fn log_batch(entries: &[(u64, &str)]) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)?;

    for (timestamp, message) in entries {
        writeln!(file, "{} - {}", timestamp, message)?;
    }

    Ok(())
}

// Timer frequency configuration
pub const TIMER_FREQUENCY_HZ: u64 = 99_990_000; // 99.99 MHz (arch_timer)

// Helper functions for time conversion
pub fn ticks_to_nanoseconds(ticks: u64) -> u64 {
    // (ticks * 1_000_000_000) / TIMER_FREQUENCY_HZ
    // Optimized: 99.99 MHz ≈ 100 MHz → each tick ≈ 10.001 ns
    ticks.saturating_mul(10001) / 1000
}

pub fn ticks_to_microseconds(ticks: u64) -> u64 {
    // (ticks * 1_000_000) / TIMER_FREQUENCY_HZ
    ticks.saturating_mul(10001) / 999_900
}

pub fn ticks_to_milliseconds(ticks: u64) -> u64 {
    // (ticks * 1000) / TIMER_FREQUENCY_HZ
    ticks / 99_990
}

/// Format elapsed time in the most appropriate unit
pub fn format_elapsed(ticks: u64) -> String {
    let ns = ticks_to_nanoseconds(ticks);

    if ns < 1_000 {
        format!("{} ns", ns)
    } else if ns < 1_000_000 {
        format!("{:.2} μs", ns as f64 / 1_000.0)
    } else if ns < 1_000_000_000 {
        format!("{:.2} ms", ns as f64 / 1_000_000.0)
    } else {
        format!("{:.3} s", ns as f64 / 1_000_000_000.0)
    }
}

/// Log elapsed time between two captures with automatic unit selection
pub fn log_elapsed(start: u64, end: u64, operation: &str) -> io::Result<()> {
    let elapsed_ticks = end.wrapping_sub(start);
    let formatted_time = format_elapsed(elapsed_ticks);

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)?;

    writeln!(
        file,
        "{} - {} completed in {} ({} ticks)",
        end, operation, formatted_time, elapsed_ticks
    )?;

    Ok(())
}
