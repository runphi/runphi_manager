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
    <locked/>
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

/// Kernel console of a Linux guest: the first UART of the machine model,
/// a PL011 (ttyAMA0) on the aarch64 virt board, a 16550 (ttyS0) on x86.
pub fn guest_console(os_arch: &str) -> &'static str {
    match os_arch {
        "aarch64" => "ttyAMA0",
        _ => "ttyS0,115200",
    }
}

/// libvirt target of the guest serial port: there is no ISA bus on the
/// aarch64 virt board, whose UART is a system-bus PL011.
pub fn serial_target_xml(os_arch: &str) -> &'static str {
    match os_arch {
        "aarch64" => "<target type='system-serial' port='0'>\n        <model name='pl011'/>\n      </target>",
        _ => "<target type='isa-serial' port='0'/>",
    }
}

pub fn config_generate(fc: &f2b::FrontendConfig) -> Result<Box<f2b::ImageConfig>, Box<dyn Error>> {
    let mut c = BackendConfig::new();
    c.name = format!("runphi-{}", fc.containerid);
    c.xml_file = fc.crundir.join("domain.xml");

    let mut config = match f2b::ImageConfig::get_from_file(&fc.mountpoint) {
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

    // CPUs for QEMU's non-vCPU threads, resolved once for this host and
    // container and stored back: Some([]) means none. cpuconf turns it into
    // <emulatorpin>, the cgroup setup and createguest read it from here.
    let emulator = cpu::emulator_cpus(fc, &config);
    if let Some(cpus) = &emulator {
        logging::log_message(
            logging::Level::Info,
            &format!(
                "Emulator threads of container {} on CPUs {}",
                fc.containerid,
                cpu::format_cpulist(cpus)
            ),
        );
    }
    config.emulator_pinning = Some(emulator.map(|cpus| cpus.into_iter().collect()).unwrap_or_default());

    // 2. Popolamento sezioni da sottomoduli
    cpu::cpuconf(fc, &config, &mut c)?;
    boot::bootconf(&config, &mut c, &is_linux)?;
    disk::diskconf(fc, &mut c, &config)?;
    network::netconf(fc, &config, &mut c)?;

    let serial_log = format!("/var/log/libvirt/qemu/runphi-{}-serial.log", fc.containerid);
    let serial_xml = format!(
        r#"<serial type='pty'>
      <log file='{}' append='on'/>
      {}
    </serial>"#,
        serial_log,
        serial_target_xml(&c.os_arch)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> BackendConfig {
        BackendConfig {
            domain_type: "kvm".to_string(),
            name: "runphi-test".to_string(),
            memory_kib: 1024 * 1024,
            vcpus: 2,
            cputune_xml: "<cputune>\n    <vcpupin vcpu='0' cpuset='2'/>\n    <vcpupin vcpu='1' cpuset='3'/>\n  </cputune>".to_string(),
            cpu_xml: "<cpu mode='host-passthrough'/>".to_string(),
            os_arch: "aarch64".to_string(),
            os_machine: "virt".to_string(),
            os_boot_xml: "<kernel>/boot/Image</kernel>\n    <cmdline>console=ttyAMA0</cmdline>".to_string(),
            features_xml: "<gic version='host'/>".to_string(),
            devices_xml: vec![
                "<interface type='user'>\n      <model type='virtio'/>\n    </interface>".to_string(),
                "<console type='pty'>\n      <target type='serial' port='0'/>\n    </console>".to_string(),
            ],
            xml_file: PathBuf::from("/tmp/domain.xml"),
        }
    }

    #[test]
    fn test_to_xml_is_well_formed() {
        let xml = sample_config().to_xml();
        let doc = roxmltree::Document::parse(&xml)
            .unwrap_or_else(|e| panic!("to_xml() produced malformed XML: {}\n{}", e, xml));

        let root = doc.root_element();
        assert_eq!(root.tag_name().name(), "domain");
        assert_eq!(root.attribute("type"), Some("kvm"));

        let child_text = |tag: &str| {
            root.children()
                .find(|n| n.has_tag_name(tag))
                .and_then(|n| n.text())
        };
        assert_eq!(child_text("name"), Some("runphi-test"));
        assert_eq!(child_text("memory"), Some("1048576"));
        assert_eq!(child_text("vcpu"), Some("2"));
    }

    #[test]
    fn test_guest_console_and_serial_target() {
        assert_eq!(guest_console("aarch64"), "ttyAMA0");
        assert_eq!(guest_console("x86_64"), "ttyS0,115200");
        assert!(serial_target_xml("aarch64").contains("type='system-serial'"));
        assert!(serial_target_xml("aarch64").contains("<model name='pl011'/>"));
        assert!(serial_target_xml("x86_64").contains("type='isa-serial'"));
    }

    #[test]
    fn test_to_xml_aarch64_serial_is_well_formed() {
        let mut c = sample_config();
        c.devices_xml.push(format!(
            "<serial type='pty'>\n      <log file='/tmp/s.log' append='on'/>\n      {}\n    </serial>",
            serial_target_xml(&c.os_arch)
        ));
        let xml = c.to_xml();
        let doc = roxmltree::Document::parse(&xml)
            .unwrap_or_else(|e| panic!("malformed XML: {}\n{}", e, xml));
        let target = doc
            .descendants()
            .find(|n| n.has_tag_name("serial"))
            .and_then(|s| s.children().find(|n| n.has_tag_name("target")))
            .expect("missing <serial><target>");
        assert_eq!(target.attribute("type"), Some("system-serial"));
        assert!(target.children().any(|n| n.has_tag_name("model") && n.attribute("name") == Some("pl011")));
    }

    #[test]
    fn test_to_xml_memory_backing_locked() {
        let xml = sample_config().to_xml();
        assert!(xml.contains("<locked/>"));
        assert!(!xml.contains("<locked\\>"));

        let doc = roxmltree::Document::parse(&xml).unwrap();
        let backing = doc
            .descendants()
            .find(|n| n.has_tag_name("memoryBacking"))
            .expect("missing <memoryBacking>");
        assert!(backing.children().any(|n| n.has_tag_name("locked")));
    }
}
