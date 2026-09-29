use std::error::Error;
use std::path::Path;

use crate::configGenerator;
use f2b;

/// Checks whether networking is enabled based on the "net" field value.
pub fn is_net_enabled(net_val: &str) -> bool {
    let trimmed = net_val.trim();
    if trimmed.is_empty() {
        return false;
    }
    !matches!(
        trimmed.to_lowercase().as_str(),
        "no" | "none" | "false" | "off" | "disabled" | "0"
    )
}

/// Detects an available host bridge or falls back to a default bridge name.
fn detect_default_bridge() -> String {
    for candidate in &["virbr0", "docker0", "xenbr0", "br0"] {
        if Path::new(&format!("/sys/class/net/{}", candidate)).exists() {
            return candidate.to_string();
        }
    }
    "virbr0".to_string()
}

/// Generates the virsh interface XML block according to the "net" specification.
pub fn generate_interface_xml(net_val: &str, netconf_val: &str) -> String {
    let trimmed = net_val.trim();
    let lower = trimmed.to_lowercase();

    if lower == "user" || lower == "slirp" {
        r#"<interface type='user'>
      <model type='virtio'/>
    </interface>"#
            .to_string()
    } else if lower.starts_with("bridge:") {
        let bridge_name = trimmed[7..].trim();
        let bridge_name = if bridge_name.is_empty() {
            detect_default_bridge()
        } else {
            bridge_name.to_string()
        };
        format!(
            r#"<interface type='bridge'>
      <source bridge='{}'/>
      <model type='virtio'/>
    </interface>"#,
            bridge_name
        )
    } else if lower == "bridge" {
        let bridge_name = if !netconf_val.trim().is_empty() {
            netconf_val.trim().to_string()
        } else {
            detect_default_bridge()
        };
        format!(
            r#"<interface type='bridge'>
      <source bridge='{}'/>
      <model type='virtio'/>
    </interface>"#,
            bridge_name
        )
    } else if lower == "network" || lower.starts_with("network:") {
        let net_name = if lower.starts_with("network:") {
            let n = trimmed[8..].trim();
            if n.is_empty() { "default" } else { n }
        } else {
            "default"
        };
        format!(
            r#"<interface type='network'>
      <source network='{}'/>
      <model type='virtio'/>
    </interface>"#,
            net_name
        )
    } else if Path::new(&format!("/sys/class/net/{}", trimmed)).exists() {
        // Direct bridge name specified (e.g. "docker0", "virbr0")
        format!(
            r#"<interface type='bridge'>
      <source bridge='{}'/>
      <model type='virtio'/>
    </interface>"#,
            trimmed
        )
    } else {
        // Default mode ("yes", "true", etc.):
        // Check if virtnetworkd socket is available; if not, prefer an existing host bridge (e.g. docker0)
        let virtnetworkd_active = Path::new("/var/run/libvirt/virtnetworkd-sock").exists()
            || Path::new("/run/libvirt/virtnetworkd-sock").exists();

        if virtnetworkd_active {
            let net_name = if !netconf_val.trim().is_empty() {
                netconf_val.trim()
            } else {
                "default"
            };
            format!(
                r#"<interface type='network'>
      <source network='{}'/>
      <model type='virtio'/>
    </interface>"#,
                net_name
            )
        } else {
            let bridge = detect_default_bridge();
            if Path::new(&format!("/sys/class/net/{}", bridge)).exists() {
                format!(
                    r#"<interface type='bridge'>
      <source bridge='{}'/>
      <model type='virtio'/>
    </interface>"#,
                    bridge
                )
            } else {
                r#"<interface type='user'>
      <model type='virtio'/>
    </interface>"#
                    .to_string()
            }
        }
    }
}

/// Populates backend network interface configuration in devices_xml if enabled by "net" in config.json.
pub fn netconf(
    _fc: &f2b::FrontendConfig,
    ic: &f2b::ImageConfig,
    c: &mut configGenerator::BackendConfig,
) -> Result<(), Box<dyn Error>> {
    if !is_net_enabled(&ic.net) {
        logging::log_message(
            logging::Level::Debug,
            &format!("Networking is disabled (net='{}')", ic.net),
        );
        return Ok(());
    }

    let interface_xml = generate_interface_xml(&ic.net, &ic.netconf);
    c.devices_xml.push(interface_xml.clone());

    logging::log_message(
        logging::Level::Info,
        &format!("Configured network interface for guest {}: net='{}'", c.name, ic.net),
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_net_enabled_disabled_values() {
        assert!(!is_net_enabled(""));
        assert!(!is_net_enabled("   "));
        assert!(!is_net_enabled("no"));
        assert!(!is_net_enabled("No"));
        assert!(!is_net_enabled("NO"));
        assert!(!is_net_enabled("none"));
        assert!(!is_net_enabled("false"));
        assert!(!is_net_enabled("off"));
        assert!(!is_net_enabled("disabled"));
        assert!(!is_net_enabled("0"));
    }

    #[test]
    fn test_is_net_enabled_enabled_values() {
        assert!(is_net_enabled("yes"));
        assert!(is_net_enabled("Yes"));
        assert!(is_net_enabled("true"));
        assert!(is_net_enabled("default"));
        assert!(is_net_enabled("bridge"));
        assert!(is_net_enabled("user"));
        assert!(is_net_enabled("virbr0"));
        assert!(is_net_enabled("docker0"));
    }

    #[test]
    fn test_generate_interface_user_slirp() {
        let xml = generate_interface_xml("user", "");
        assert!(xml.contains("type='user'"));
        assert!(xml.contains("<model type='virtio'/>"));

        let xml_slirp = generate_interface_xml("slirp", "");
        assert!(xml_slirp.contains("type='user'"));
    }

    #[test]
    fn test_generate_interface_bridge() {
        let xml = generate_interface_xml("bridge:mybr0", "");
        assert!(xml.contains("type='bridge'"));
        assert!(xml.contains("source bridge='mybr0'"));
        assert!(xml.contains("<model type='virtio'/>"));

        let xml_conf = generate_interface_xml("bridge", "custombr");
        assert!(xml_conf.contains("source bridge='custombr'"));
    }

    #[test]
    fn test_generate_interface_network() {
        let xml = generate_interface_xml("network:custom-net", "");
        assert!(xml.contains("type='network'"));
        assert!(xml.contains("source network='custom-net'"));

        let xml_net = generate_interface_xml("network", "");
        assert!(xml_net.contains("type='network'"));
        assert!(xml_net.contains("source network='default'"));
    }

    #[test]
    fn test_netconf_integration() {
        let fc = f2b::FrontendConfig::new();
        let mut ic = f2b::ImageConfig {
            cpio: String::new(),
            os_var: "linux".to_string(),
            kernel: String::new(),
            ramdisk: String::new(),
            inmate: "/boot/Image".to_string(),
            dtb: String::new(),
            initrd: String::new(),
            netconf: String::new(),
            disk_type: String::new(),
            disk_image: String::new(),
            disk_size: 0,
            starting_vaddress: String::new(),
            memory: 1024,
            net: "no".to_string(),
            rpu_req: false,
            vcpus: 1,
            vcpu_pinning: Vec::new(),
            isolcpu: String::new(),
            nohz_full: String::new(),
            steer_irq: None,
        };

        let mut c = configGenerator::BackendConfig::new();
        c.name = "test-guest".to_string();

        // Disabled case
        netconf(&fc, &ic, &mut c).unwrap();
        assert!(c.devices_xml.is_empty());

        // Enabled case
        ic.net = "yes".to_string();
        netconf(&fc, &ic, &mut c).unwrap();
        assert_eq!(c.devices_xml.len(), 1);
        assert!(c.devices_xml[0].contains("<model type='virtio'/>"));
    }
}
