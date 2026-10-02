// KVM tick source: the host CPU's own counter, read directly from user
// space, so it needs neither /dev/mem (Jailhouse) nor a kernel module
// (Xen's /dev/arm_timer).
//
// - x86_64: the TSC (lfence; rdtsc).
// - aarch64: the generic timer's virtual counter CNTVCT_EL0, which Linux
//   lets user space read (the vDSO relies on it). On a KVM host the
//   virtual offset (CNTVOFF_EL2) is 0, so this is the same system counter
//   the Jailhouse and Xen backends read, at the same frequency (99.99 MHz
//   on the Zynq UltraScale+ / Kria boards) that logging::timer assumes
//   when converting ticks to time.

use logging::timer::TickSource;

pub struct KvmTickSource;

impl KvmTickSource {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self)
    }
}

impl TickSource for KvmTickSource {
    #[inline(always)]
    fn read_ticks(&self) -> u64 {
        #[cfg(target_arch = "x86_64")]
        {
            let low: u32;
            let high: u32;
            unsafe {
                std::arch::asm!(
                    "lfence",
                    "rdtsc",
                    out("eax") low,
                    out("edx") high,
                    options(nomem, nostack, preserves_flags)
                );
            }
            ((high as u64) << 32) | (low as u64)
        }

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

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            0
        }
    }
}

pub fn install() -> std::io::Result<()> {
    logging::timer::initialize_with(Box::new(KvmTickSource::new()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    #[test]
    fn test_ticks_advance() {
        let t = KvmTickSource::new().unwrap();
        let a = t.read_ticks();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = t.read_ticks();
        assert!(a > 0 && b > a, "counter did not advance: {} -> {}", a, b);
    }
}
