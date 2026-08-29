use std::error::Error;
use std::fs;
use std::path::PathBuf;

// Struttura minimale per accumulare gli argomenti CLI di QEMU
#[derive(Debug, Default)]
pub struct BackendConfig {
    pub args: Vec<String>,
    pub args_file: PathBuf,
}

impl BackendConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_arg<S: Into<String>>(&mut self, flag: &str, val: S) {
        self.args.push(flag.to_string());
        self.args.push(val.into());
    }

    pub fn add_flag(&mut self, flag: &str) {
        self.args.push(flag.to_string());
    }
}

pub fn config_generate(fc: &f2b::FrontendConfig) -> Result<Box<f2b::ImageConfig>, Box<dyn Error>> {
    let mut c = BackendConfig::new();
    c.args_file = fc.crundir.join("qemu.args");

    let mut config = Box::new(f2b::ImageConfig::get_from_file(&fc.mountpoint)?);
    let is_linux = config.os_var.eq_ignore_ascii_case("linux");

    //TODO(lorenzo): Qua si deve capire se va bene anche per arm
    #[cfg(target_arch = "aarch64")]
    {
        c.add_arg("-M", "virt");
        c.add_arg("-cpu", "host");
    }
    #[cfg(target_arch = "x86_64")]
    {
        c.add_arg("-M", "q35");
        c.add_arg("-cpu", "host");
    }

    c.add_flag("-enable-kvm");
    //c.add_arg("-display", "none");
    //c.add_arg("-monitor", "none");
    c.add_flag("-nographic");
    c.add_flag("-no-reboot");
    c.add_arg("-d", "int,guest_errors");
    
    let debug_file = fc.crundir.join("qemu-cpu-debug.log");

    let console_sock = fc.crundir.join("console.sock");
    c.add_arg("-serial", format!("unix:{},server,nowait", console_sock.display()));

    c.add_arg("-D", format!("{}", debug_file.display()));
    c.add_flag("-S"); 
    
    let console_file = fc.crundir.join("console.log");
    //c.add_arg("-serial", format!("file:{}", console_file.display()));
    c.add_arg("-serial", "stdio");

    let period = fc.jsonconfig["linux"]["resources"]["cpu"]["period"]
        .as_f64()
        .unwrap_or(10000.0);
    //TODO(lorenzo): Default 20000/10000 = 2 per zephyr
    let quota = fc.jsonconfig["linux"]["resources"]["cpu"]["quota"]
        .as_f64()
        .unwrap_or(20000.0);
    
    let cpus = quota / period;

    c.add_arg("-smp", format!("{}", cpus));

    // 4. Calcolo Memoria RAM (MB)
    let default_mb: u64 = if is_linux { 1024 } else { 32 };
    let mem_mb = if config.memory > 0 {
        config.memory
    } else {
        fc.jsonconfig["linux"]["resources"]["memory"]["limit"]
            .as_u64()
            .map(|b| b / (1024 * 1024))
            .filter(|&mb| mb > 0)
            .unwrap_or(default_mb)
    };
    c.add_arg("-m", format!("{}M", mem_mb));

    // 5. Configurazione Socket QMP
    let qmp_socket = fc.crundir.join(format!("{}-qmp.sock", fc.containerid));
    c.add_arg("-qmp", format!("unix:{},server,nowait", qmp_socket.display()));

    //TODO(lorenzo): Si deve testare qui se va bene anche per un linux inmate
    if is_linux {
        if !config.inmate.is_empty() {
            c.add_arg("-kernel", &config.inmate);
        }
        if !config.ramdisk.is_empty() {
            c.add_arg("-initrd", &config.ramdisk);
        }
        if !config.dtb.is_empty() {
            c.add_arg("-dtb", &config.dtb);
        }
        c.add_arg("-append", "console=ttyS0,115200");
    } else {
        // Bare-metal / RTOS (Zephyr, FreeRTOS ELF/BIN)
        c.add_arg("-kernel", &config.inmate);
    }

    // 7. Scrittura su disco: un argomento per riga
    let file_content = c.args.join("\n");
    fs::write(&c.args_file, file_content)?;
    Ok(config)
}
