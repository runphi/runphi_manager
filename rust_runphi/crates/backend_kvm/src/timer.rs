// KVM tick source: the host CPU's own counter, read directly from user
// space, so it needs neither /dev/mem (Jailhouse) nor a kernel module
// (Xen's /dev/arm_timer).
//
// - x86_64: the TSC (lfence; rdtsc). Its frequency is measured against
//   CLOCK_MONOTONIC the first time ticks are converted to time.
// - aarch64: the generic timer's virtual counter CNTVCT_EL0, which Linux
//   lets user space read (the vDSO relies on it). On a KVM host the
//   virtual offset (CNTVOFF_EL2) is 0, so this is the same system counter
//   the Jailhouse and Xen backends read; its frequency is CNTFRQ_EL0
//   (99.99 MHz on the Zynq UltraScale+ / Kria boards, 62.5 MHz on QEMU).

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

    fn frequency_hz(&self) -> u64 {
        #[cfg(target_arch = "x86_64")]
        {
            tsc_frequency_hz()
        }

        #[cfg(target_arch = "aarch64")]
        {
            logging::timer::arm_counter_frequency_hz()
        }

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            0
        }
    }
}

// TSC ticks per second, counted over 20 ms of CLOCK_MONOTONIC (std's
// Instant) the first time it is needed: no register holds it, and runphi
// runs once per OCI command, so it must not delay the ones that never
// convert ticks to time.
#[cfg(target_arch = "x86_64")]
fn tsc_frequency_hz() -> u64 {
    static FREQUENCY: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *FREQUENCY.get_or_init(|| {
        let tsc = KvmTickSource;
        let (t0, c0) = (std::time::Instant::now(), tsc.read_ticks());
        std::thread::sleep(std::time::Duration::from_millis(20));
        let (c1, elapsed) = (tsc.read_ticks(), t0.elapsed());
        let ns = elapsed.as_nanos().max(1);
        (c1.wrapping_sub(c0) as u128 * 1_000_000_000 / ns) as u64
    })
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

    // 100 ms of sleep must convert to about 100 ms with the reported frequency.
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    #[test]
    fn test_frequency_converts_ticks_to_time() {
        let t = KvmTickSource::new().unwrap();
        let f = t.frequency_hz() as u128;
        assert!(f > 1_000_000, "implausible frequency {} Hz", f);
        let a = t.read_ticks();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let ms = (t.read_ticks() - a) as u128 * 1000 / f;
        assert!((100..150).contains(&ms), "100 ms measured as {} ms", ms);
    }
}
