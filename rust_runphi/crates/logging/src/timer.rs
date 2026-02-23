//*********************************************
// Authors:
// Francesco Boccola (francesco.boccola@unina.it)
//*********************************************

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::ptr;
use std::sync::Once;
use libc::{off_t, MAP_FAILED, MAP_SHARED, PROT_READ, O_RDWR, O_SYNC};

const TIMER_ADDR: usize = 0xff250000;
const PAGE_SIZE: usize = 4096;
const LOG_PATH: &str = "/usr/share/runPHI/log.txt"; //Change to your desired log file path

static mut TIMER_PTR: *const u32 = ptr::null();
static INIT: Once = Once::new();

unsafe fn init_timer() -> io::Result<()> {
    let mem_fd = libc::open("/dev/mem\0".as_ptr() as *const libc::c_char, O_RDWR | O_SYNC);
    if mem_fd < 0 {
        return Err(io::Error::last_os_error());
    }

    let page_addr = TIMER_ADDR & !(PAGE_SIZE - 1);
    let page_offset = TIMER_ADDR - page_addr;

    let mapped = libc::mmap(
        ptr::null_mut(),
        PAGE_SIZE,
        PROT_READ,
        MAP_SHARED,
        mem_fd,
        page_addr as off_t,
    );

    libc::close(mem_fd);

    if mapped == MAP_FAILED {
        return Err(io::Error::last_os_error());
    }

    TIMER_PTR = (mapped as *mut u8).add(page_offset) as *const u32;
    Ok(())
}

/// Explicitly initialize the timer at program start (optional but recommended)
pub fn initialize() -> io::Result<()> {
    unsafe {
        let mut result = Ok(());
        INIT.call_once(|| {
            if let Err(e) = init_timer() {
                result = Err(e);
            }
        });
        result
    }
}

/// Log timestamp with message
#[inline(never)]
pub fn log_phase(message: &str) -> io::Result<()> {
    INIT.call_once(|| {
        unsafe {
            if let Err(e) = init_timer() {
                eprintln!("Failed to initialize timer: {}", e);
            }
        }
    });

    let timestamp = unsafe {
        if TIMER_PTR.is_null() {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "Timer not initialized"));
        }
        ptr::read_volatile(TIMER_PTR)
    };

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)?;
    
    writeln!(file, "0x{:08X} - {}", timestamp, message)?;
    
    Ok(())
}

/// Log a pre-captured timestamp with message
#[inline(never)]
pub fn log_phase_at(timestamp: u32, message: &str) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)?;
    
    writeln!(file, "0x{:08X} - {}", timestamp, message)?;
    
    Ok(())
}

/// Ultra-fast timestamp capture without file I/O
#[inline(always)]
pub fn capture() -> u32 {
    unsafe {
        INIT.call_once(|| {
            let _ = init_timer();
        });
        
        if TIMER_PTR.is_null() {
            0
        } else {
            ptr::read_volatile(TIMER_PTR)
        }
    }
}

/// Batch write multiple timestamps
pub fn log_batch(entries: &[(u32, &str)]) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)?;
    
    for (timestamp, message) in entries {
        writeln!(file, "0x{:08X} - {}", timestamp, message)?;
    }
    
    Ok(())
}

// Timer frequency configuration
pub const TIMER_FREQUENCY_HZ: u64 = 99_990_000; // 99.99 MHz (from arch_timer)

// Helper functions for time conversion
pub fn ticks_to_nanoseconds(ticks: u32) -> u64 {
    // More accurate: (ticks * 1_000_000_000) / TIMER_FREQUENCY_HZ
    // Optimized version using the fact that 99.99 MHz ≈ 100 MHz with 0.01% error
    (ticks as u64 * 10001) / 1000  // Each tick ≈ 10.001 ns
}

pub fn ticks_to_microseconds(ticks: u32) -> u64 {
    // (ticks * 1_000_000) / TIMER_FREQUENCY_HZ
    (ticks as u64 * 10001) / 999900  // ≈ ticks / 99.99
}

pub fn ticks_to_milliseconds(ticks: u32) -> u64 {
    // (ticks * 1000) / TIMER_FREQUENCY_HZ
    (ticks as u64) / 99990  // 99,990 ticks = 1 millisecond
}

/// Format elapsed time in the most appropriate unit
pub fn format_elapsed(ticks: u32) -> String {
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
pub fn log_elapsed(start: u32, end: u32, operation: &str) -> io::Result<()> {
    let elapsed_ticks = end.wrapping_sub(start);
    let formatted_time = format_elapsed(elapsed_ticks);
    
    // Use the 'end' timestamp for logging, not a new capture
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)?;
    
    writeln!(file, "0x{:08X} - {} completed in {} ({} ticks)", 
             end, operation, formatted_time, elapsed_ticks)?;
    
    Ok(())
}

// ===== SETUP INSTRUCTIONS =====

// 1. logging/src/lib.rs
// Add this to expose the timer module:
/*
pub mod timer;
*/

// 2. logging/Cargo.toml
// Add this dependency:
/*
[dependencies]
libc = "0.2"
*/

// 3. In other crates that need timing (e.g., backend_jailhouse/Cargo.toml):
/*
[dependencies]
logging = { path = "../logging" }
*/

// 4. runphi/src/main.rs
// Initialize at program start:
/*
use logging::timer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize timer early to catch any errors
    timer::initialize()?;
    println!("Timer initialized successfully");
    
    // Your existing main code...
    Ok(())
}
*/

// ===== USAGE EXAMPLES =====

// Example 1: Simple with automatic formatting (RECOMMENDED)
/*
use logging::timer;

pub fn create_container() -> Result<()> {
    let start = timer::capture();
    
    // ... container creation code ...
    
    let end = timer::capture();
    timer::log_elapsed(start, end, "Container creation")?;
    // Logs: "Container creation completed in 45.23 ms (4522317 ticks)"
    
    Ok(())
}
*/

// Example 2: Manual with specific units
/*
use logging::timer;

pub fn process_data() -> Result<()> {
    let start = timer::capture();
    
    // ... your processing code ...
    
    let end = timer::capture();
    let elapsed_ticks = end.wrapping_sub(start);
    let elapsed_us = timer::ticks_to_microseconds(elapsed_ticks);
    
    timer::log_phase(&format!("Data processed in {} μs", elapsed_us))?;
    
    Ok(())
}
*/

// Example 3: With multiple time units
/*
use logging::timer;

pub fn complex_operation() -> Result<()> {
    let start = timer::capture();
    
    // ... complex operation ...
    
    let end = timer::capture();
    let elapsed_ticks = end.wrapping_sub(start);
    let elapsed_ms = timer::ticks_to_milliseconds(elapsed_ticks);
    let elapsed_us = timer::ticks_to_microseconds(elapsed_ticks);
    
    timer::log_phase(&format!(
        "Operation completed in {:.2} ms ({} μs, {} ticks)", 
        elapsed_ms as f64, elapsed_us, elapsed_ticks
    ))?;
    
    Ok(())
}
*/

// Example 4: Using the auto-formatter
/*
use logging::timer;

pub fn network_request() -> Result<()> {
    let start = timer::capture();
    
    // ... network operation ...
    
    let end = timer::capture();
    let elapsed_ticks = end.wrapping_sub(start);
    
    timer::log_phase(&format!(
        "Network request completed in {}", 
        timer::format_elapsed(elapsed_ticks)
    ))?;
    // Automatically chooses ns, μs, ms, or s based on duration
    
    Ok(())
}
*/

// Example 5: Performance profiling with multiple phases
/*
use logging::timer;

pub fn profile_container_creation() -> Result<()> {
    let t0 = timer::capture();
    
    // Phase 1: Initialize
    initialize_container()?;
    let t1 = timer::capture();
    
    // Phase 2: Configure
    configure_container()?;
    let t2 = timer::capture();
    
    // Phase 3: Start
    start_container()?;
    let t3 = timer::capture();
    
    // Log all phases
    timer::log_phase(&format!("Initialization: {}", 
        timer::format_elapsed(t1.wrapping_sub(t0))))?;
    timer::log_phase(&format!("Configuration: {}", 
        timer::format_elapsed(t2.wrapping_sub(t1))))?;
    timer::log_phase(&format!("Startup: {}", 
        timer::format_elapsed(t3.wrapping_sub(t2))))?;
    timer::log_phase(&format!("Total: {}", 
        timer::format_elapsed(t3.wrapping_sub(t0))))?;
    
    Ok(())
}
*/

// Example 6: High-performance batch logging (minimal I/O impact)
/*
use logging::timer;

pub fn critical_section() -> Result<()> {
    // Capture timestamps without file I/O during critical section
    let mut timestamps = Vec::new();
    
    timestamps.push((timer::capture(), "Critical: Phase 1 start"));
    // ... time-critical code ...
    timestamps.push((timer::capture(), "Critical: Phase 1 end"));
    
    timestamps.push((timer::capture(), "Critical: Phase 2 start"));
    // ... more critical code ...
    timestamps.push((timer::capture(), "Critical: Phase 2 end"));
    
    // Write all timestamps after critical section
    timer::log_batch(&timestamps)?;
    
    Ok(())
}
*/

// Example 7: Inline timing for debugging
/*
use logging::timer;

pub fn debug_slow_function() -> Result<()> {
    let t = timer::capture();
    step_1()?;
    timer::log_phase(&format!("Step 1: {}", timer::format_elapsed(timer::capture().wrapping_sub(t))))?;
    
    let t = timer::capture();
    step_2()?;
    timer::log_phase(&format!("Step 2: {}", timer::format_elapsed(timer::capture().wrapping_sub(t))))?;
    
    let t = timer::capture();
    step_3()?;
    timer::log_phase(&format!("Step 3: {}", timer::format_elapsed(timer::capture().wrapping_sub(t))))?;
    
    Ok(())
}
*/

// Example 8: Using log_phase_at() for synchronized timestamps
/*
use logging::timer;

pub fn process_with_checkpoints() -> Result<()> {
    let start = timer::capture();
    
    // Do some work...
    perform_initialization()?;
    
    let checkpoint1 = timer::capture();
    timer::log_elapsed(start, checkpoint1, "Initialization")?;
    timer::log_phase_at(checkpoint1, "Initialization complete")?;  // Same timestamp
    
    // More work...
    load_configuration()?;
    
    let checkpoint2 = timer::capture();
    timer::log_elapsed(checkpoint1, checkpoint2, "Configuration loading")?;
    timer::log_phase_at(checkpoint2, "Configuration loaded")?;  // Same timestamp
    timer::log_phase_at(checkpoint2, "Ready to start processing")?;  // Multiple logs at same time
    
    // Final work...
    process_data()?;
    
    let end = timer::capture();
    timer::log_elapsed(checkpoint2, end, "Data processing")?;
    timer::log_elapsed(start, end, "Total execution")?;
    timer::log_phase_at(end, "All operations complete")?;
    
    Ok(())
}
*/