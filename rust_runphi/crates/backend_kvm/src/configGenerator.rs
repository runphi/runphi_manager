use std::error::Error;
use std::fs;
use std::path::PathBuf;

pub mod boot;
pub mod cpu;
pub mod disk;
pub mod network;

#[derive(Debug, Default)]
pub struct BackendConfig {
    pub domain_type: String,      // "kvm" oppure "qemu"
    pub name: String,             // Nome del container/dominio
    pub memory_kib: u64,          // Memoria RAM in KiB
    pub vcpus: u32,               // Numero di vCPU
    pub cputune_xml: String,      // Tag <cputune> con i mapping <vcpupin>
    pub cpu_xml: String,          // Tag <cpu mode='...'>
    pub os_arch: String,          // "x86_64" o "aarch64"
    pub os_machine: String,       // "q35" o "virt"
    pub os_boot_xml: String,      // <kernel>, <initrd>, <cmdline>, <dtb>
    pub features_xml: String,     // <acpi/>, <apic/>, <gic/>
    pub devices_xml: Vec<String>, // Dischi, interfacce di rete, seriali
    pub xml_file: PathBuf,        // Percorso di destinazione (es. domain.xml)
}

impl BackendConfig {
    pub fn new() -> Self {
        Self {
            domain_type: "kvm".to_string(),
            os_arch: std::env::consts::ARCH.to_string(),
            ..Default::default()
        }
    }

    /// Assembla tutti i blocchi parziali in un documento Domain XML valido per virsh
    pub fn to_xml(&self) -> String {
        let devices = self.devices_xml.join("\n    ");
//TODO(lorenzo): La parte di memory backing non è attualmente configurabile via config.json o per passaggio di parametri da docker run
        format!(
            r#"<domain type='{}'>
  <name>{}</name>
  <memory unit='KiB'>{}</memory>
  <vcpu placement='static'>{}</vcpu>
  {}
  <os>
    <type arch='{}' machine='{}'>hvm</type>
    {}
  </os>
  <features>
    {}
  </features>
  {}
  <seclabel type='static' model='dac' relabel='no'>
    <label>root:root</label>
  </seclabel>
  <memoryBacking>
   <locked\>
  </memoryBacking>
  <on_poweroff>destroy</on_poweroff>
  <on_reboot>destroy</on_reboot>
  <on_crash>destroy</on_crash>
  <devices>
    <emulator>/usr/bin/qemu-system-{}</emulator>
    {}
  </devices>
</domain>"#,
            self.domain_type,
            self.name,
            self.memory_kib,
            self.vcpus,
            self.cputune_xml,
            self.os_arch,
            self.os_machine,
            self.os_boot_xml,
            self.features_xml,
            self.cpu_xml,
            self.os_arch,
            devices
        )
    }
}

pub fn config_generate(fc: &f2b::FrontendConfig) -> Result<Box<f2b::ImageConfig>, Box<dyn Error>> {
    let mut c = BackendConfig::new();
    c.name = format!("runphi-{}", fc.containerid);
    c.xml_file = fc.crundir.join("domain.xml");

    let config = match f2b::ImageConfig::get_from_file(&fc.mountpoint) {
        Ok(cfg) => Box::new(cfg),
        Err(e) => {
            logging::log_message(
                logging::Level::Error,
                &format!("Failed to read boot config from {}: {}", fc.mountpoint.display(), e),
            );
            return Err(e);
        }
    };
    let is_linux = config.os_var.eq_ignore_ascii_case("linux");

    // 1. Calcolo Memoria RAM (convertita in KiB per Libvirt)
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
    c.memory_kib = mem_mb * 1024;

    // 2. Popolamento sezioni da sottomoduli
    cpu::cpuconf(fc, &config, &mut c)?;
    boot::bootconf(&config, &mut c, &is_linux)?;
    disk::diskconf(fc, &mut c, &config)?;
    network::netconf(fc, &config, &mut c)?;

    let serial_log = format!("/var/log/libvirt/qemu/runphi-{}-serial.log", fc.containerid);
    let serial_xml = format!(
        r#"<serial type='pty'>
      <log file='{}' append='on'/>
      <target type='isa-serial' port='0'/>
    </serial>"#,
        serial_log
    );
    c.devices_xml.push(serial_xml);

    // Console primaria agganciata al PTY
    let console_xml = r#"<console type='pty'>
      <target type='serial' port='0'/>
    </console>"#.to_string();
    c.devices_xml.push(console_xml);


    // 4. Scrittura del file XML finale su disco
    let xml_content = c.to_xml();

    fs::write(&c.xml_file, xml_content)?;
    Ok(config)
}
