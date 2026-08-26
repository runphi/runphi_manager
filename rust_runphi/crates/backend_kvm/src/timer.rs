use logging::timer::TickSource;

pub struct X86TscTickSource;

impl X86TscTickSource {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self)
    }
}

impl TickSource for X86TscTickSource {
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

        #[cfg(not(target_arch = "x86_64"))]
        {
            0
        }
    }
}

pub fn install() -> std::io::Result<()> {
    logging::timer::initialize_with(Box::new(X86TscTickSource::new()?))
}
