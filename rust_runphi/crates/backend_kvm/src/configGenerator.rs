use std::error::Error;
use std::fs;
use std::path::PathBuf;

pub mod cpu;
pub mod boot;

// NOTE(lorenzo): Minimal structure to organize QEMU flags and args
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

    let _ = cpu::cpuconf(&fc,&config, &mut c);
    let _ = boot::bootconf(&config, &mut c, &is_linux);

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

    

    // 7. Scrittura su disco: un argomento per riga
    let file_content = c.args.join("\n");
    fs::write(&c.args_file, file_content)?;
    Ok(config)
}
