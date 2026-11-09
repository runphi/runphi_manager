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
    // Ticks per second of read_ticks(), 0 if unknown. Called only to convert
    // ticks to time, so it may be slow the first time (e.g. a calibration).
    fn frequency_hz(&self) -> u64;
}

// Frequency of the ARM generic timer's counter, CNTFRQ_EL0, which Linux lets
// userspace read: the tick rate of CNTVCT_EL0/CNTPCT_EL0 and of the system
// counter that drives them (e.g. 99.99 MHz on the ZynqMP, 62.5 MHz on QEMU's
// virt board).
#[cfg(target_arch = "aarch64")]
pub fn arm_counter_frequency_hz() -> u64 {
    let freq: u64;
    unsafe {
        std::arch::asm!(
            "mrs {}, cntfrq_el0",
            out(reg) freq,
            options(nomem, nostack, preserves_flags)
        );
    }
    freq
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

// Ticks per second of the installed source, 0 if there is none or its
// frequency is unknown (the conversions below then return 0, as capture()).
pub fn frequency_hz() -> u64 {
    SOURCE.get().map_or(0, |src| src.frequency_hz())
}

// ticks * units_per_second / frequency, in 128 bits so that it cannot overflow.
fn ticks_to_units(ticks: u64, frequency_hz: u64, units_per_second: u64) -> u64 {
    if frequency_hz == 0 {
        return 0;
    }
    (ticks as u128 * units_per_second as u128 / frequency_hz as u128) as u64
}

pub fn ticks_to_nanoseconds(ticks: u64) -> u64 {
    ticks_to_units(ticks, frequency_hz(), 1_000_000_000)
}

pub fn ticks_to_microseconds(ticks: u64) -> u64 {
    ticks_to_units(ticks, frequency_hz(), 1_000_000)
}

pub fn ticks_to_milliseconds(ticks: u64) -> u64 {
    ticks_to_units(ticks, frequency_hz(), 1_000)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_convert_with_the_source_frequency() {
        // ZynqMP system counter: 99.99 MHz
        assert_eq!(ticks_to_units(99_990_000, 99_990_000, 1_000_000_000), 1_000_000_000);
        assert_eq!(ticks_to_units(9_999, 99_990_000, 1_000_000), 100);
        // QEMU virt (cortex-a53): 62.5 MHz, 16 ns per tick
        assert_eq!(ticks_to_units(1, 62_500_000, 1_000_000_000), 16);
        assert_eq!(ticks_to_units(62_500_000, 62_500_000, 1_000), 1_000);
        // a 3 GHz TSC: a large count does not overflow
        assert_eq!(ticks_to_units(u64::MAX / 2, 3_000_000_000, 1_000_000_000), u64::MAX / 6);
    }

    #[test]
    fn unknown_frequency_converts_to_zero() {
        assert_eq!(ticks_to_units(12345, 0, 1_000_000_000), 0);
    }
}
