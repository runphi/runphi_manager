//*********************************************
// Authors:
// Francesco Boccola (francesco.boccola@unina.it)
//*********************************************
//
// Hypervisor-independent half of the timer.
//
// The actual tick source (mmap'd MMIO under Jailhouse, /dev/arm_timer
// under Xen, etc.) lives in each backend crate and is installed at
// program start via `initialize_with`. After that, the rest of the
// codebase reads ticks through `capture()` and the convenience
// helpers below, without knowing which hypervisor is underneath.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::sync::OnceLock;

use crate::LOG_PATH;

// Backend-provided source of monotonic ticks.
// Implementations must be cheap to call and safe to use from any thread.
pub trait TickSource: Send + Sync {
    fn read_ticks(&self) -> u64;
}

static SOURCE: OnceLock<Box<dyn TickSource>> = OnceLock::new();

// Install the platform tick source. Call exactly once at program start
// (typically from main, with the source built by the active backend).
// Returns AlreadyExists if called twice.
pub fn initialize_with(source: Box<dyn TickSource>) -> io::Result<()> {
    SOURCE.set(source).map_err(|_| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "timer source already installed",
        )
    })
}

// Read the current tick count. Returns 0 if no source has been installed,
// so timing helpers degrade gracefully instead of panicking.
#[inline(always)]
pub fn capture() -> u64 {
    match SOURCE.get() {
        Some(src) => src.read_ticks(),
        None => 0,
    }
}

#[inline(never)]
pub fn log_phase(message: &str) -> io::Result<()> {
    let timestamp = capture();
    let mut file = OpenOptions::new().create(true).append(true).open(LOG_PATH)?;
    writeln!(file, "{} - {}", timestamp, message)?;
    Ok(())
}

#[inline(never)]
pub fn log_phase_at(timestamp: u64, message: &str) -> io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(LOG_PATH)?;
    writeln!(file, "{} - {}", timestamp, message)?;
    Ok(())
}

pub fn log_batch(entries: &[(u64, &str)]) -> io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(LOG_PATH)?;
    for (timestamp, message) in entries {
        writeln!(file, "{} - {}", timestamp, message)?;
    }
    Ok(())
}

// arch_timer on the target board (same frequency for both backends).
pub const TIMER_FREQUENCY_HZ: u64 = 99_990_000;

pub fn ticks_to_nanoseconds(ticks: u64) -> u64 {
    // Optimized: 99.99 MHz ≈ 100 MHz → each tick ≈ 10.001 ns
    ticks.saturating_mul(10001) / 1000
}

pub fn ticks_to_microseconds(ticks: u64) -> u64 {
    ticks.saturating_mul(10001) / 999_900
}

pub fn ticks_to_milliseconds(ticks: u64) -> u64 {
    ticks / 99_990
}

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

pub fn log_elapsed(start: u64, end: u64, operation: &str) -> io::Result<()> {
    let elapsed_ticks = end.wrapping_sub(start);
    let formatted_time = format_elapsed(elapsed_ticks);
    let mut file = OpenOptions::new().create(true).append(true).open(LOG_PATH)?;
    writeln!(
        file,
        "{} - {} completed in {} ({} ticks)",
        end, operation, formatted_time, elapsed_ticks
    )?;
    Ok(())
}
