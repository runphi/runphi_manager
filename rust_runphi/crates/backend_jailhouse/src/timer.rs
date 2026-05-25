//*********************************************
// Authors:
// Francesco Boccola (francesco.boccola@unina.it)
//*********************************************
//
// Jailhouse tick source.
//
// The ARM physical timer counter is exposed at a fixed MMIO address
// on the target board; under Jailhouse Dom0 we map a single page of
// /dev/mem and read it with volatile loads. The pointer is owned by
// `JailhouseTickSource` for the rest of the process lifetime.

use std::io;
use std::ptr;

use libc::{off_t, MAP_FAILED, MAP_SHARED, O_RDWR, O_SYNC, PROT_READ};
use logging::timer::TickSource;

const TIMER_ADDR: usize = 0xff250000;
const PAGE_SIZE: usize = 4096;

pub struct JailhouseTickSource {
    // Address of the 32-bit counter register inside the mapped page.
    // Stored as usize to dodge raw-pointer Send/Sync complaints; we
    // re-cast on every read.
    counter: usize,
}

// The MMIO page is read-only and dereferenced via volatile reads;
// concurrent access from multiple threads is fine.
unsafe impl Send for JailhouseTickSource {}
unsafe impl Sync for JailhouseTickSource {}

impl JailhouseTickSource {
    pub fn new() -> io::Result<Self> {
        unsafe {
            let mem_fd = libc::open(
                "/dev/mem\0".as_ptr() as *const libc::c_char,
                O_RDWR | O_SYNC,
            );
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

            let counter = (mapped as *mut u8).add(page_offset) as usize;
            Ok(Self { counter })
        }
    }
}

impl TickSource for JailhouseTickSource {
    #[inline(always)]
    fn read_ticks(&self) -> u64 {
        // Zero-extend the 32-bit counter to the unified u64 API.
        unsafe { ptr::read_volatile(self.counter as *const u32) as u64 }
    }
}

// Convenience: build the source and install it into the logging crate.
// Called once at program start from main.
pub fn install() -> io::Result<()> {
    let src = JailhouseTickSource::new()?;
    logging::timer::initialize_with(Box::new(src))
}
