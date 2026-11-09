//*********************************************
// Authors:
// Francesco Boccola (francesco.boccola@unina.it)
//*********************************************
//
// Jailhouse tick source.
//
// The generic timer's virtual counter CNTVCT_EL0, which Linux lets user
// space read (the vDSO relies on it), at its frequency CNTFRQ_EL0. In the
// root cell the virtual offset (CNTVOFF_EL2) is 0, so this is the system
// counter itself, 64 bits wide: on the ZynqMP its low 32 bits are the value
// of IOU_SCNTR at 0xff250000, which this source used to map from /dev/mem.

use logging::timer::TickSource;

pub struct JailhouseTickSource;

impl JailhouseTickSource {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self)
    }
}

impl TickSource for JailhouseTickSource {
    #[inline(always)]
    fn read_ticks(&self) -> u64 {
        #[cfg(target_arch = "aarch64")]
        {
            let ticks: u64;
            unsafe {
                // isb: do not let the read be speculated ahead of the
                // preceding instructions, as the kernel's vDSO does.
                std::arch::asm!(
                    "isb",
                    "mrs {}, cntvct_el0",
                    out(reg) ticks,
                    options(nomem, nostack, preserves_flags)
                );
            }
            ticks
        }

        #[cfg(not(target_arch = "aarch64"))]
        {
            0
        }
    }

    fn frequency_hz(&self) -> u64 {
        #[cfg(target_arch = "aarch64")]
        {
            logging::timer::arm_counter_frequency_hz()
        }

        #[cfg(not(target_arch = "aarch64"))]
        {
            0
        }
    }
}

// Convenience: build the source and install it into the logging crate.
// Called once at program start from main.
pub fn install() -> std::io::Result<()> {
    let src = JailhouseTickSource::new()?;
    logging::timer::initialize_with(Box::new(src))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_arch = "aarch64")]
    #[test]
    fn test_ticks_advance() {
        let t = JailhouseTickSource::new().unwrap();
        let a = t.read_ticks();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = t.read_ticks();
        assert!(a > 0 && b > a, "counter did not advance: {} -> {}", a, b);
    }

    // 100 ms of sleep must convert to about 100 ms with the reported frequency.
    #[cfg(target_arch = "aarch64")]
    #[test]
    fn test_frequency_converts_ticks_to_time() {
        let t = JailhouseTickSource::new().unwrap();
        let f = t.frequency_hz() as u128;
        assert!(f > 1_000_000, "implausible frequency {} Hz", f);
        let a = t.read_ticks();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let ms = (t.read_ticks() - a) as u128 * 1000 / f;
        assert!((100..150).contains(&ms), "100 ms measured as {} ms", ms);
    }
}
