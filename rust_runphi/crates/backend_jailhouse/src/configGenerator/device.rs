//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (francesco.boccola@unina.it)
//*********************************************

use crate::configGenerator;
use regex::Regex;
//use std::collections::HashSet;
//use std::collections::HashMap;

use std::error::Error;
//use std::fs::{File, OpenOptions};
//use std::io::{self, Read, Write};
use std::path::Path;
//use f2b;
use crate::configGenerator::templates::*;


use f2b::paths::WORKPATH;

// The interrupt that the hypervisor raises for the INTx of ivshmem device
// 00:<device>.0 of a cell (hypervisor/arch/arm-common/ivshmem.c: 32 +
// vpci_irq_base + pin - 1, the pin rotated by the device number)
pub fn ivshmem_intx(vpci_irq_base: u32, device: u32) -> u32 {
    let pin = (device & 0x3) + 1;
    32 + vpci_irq_base + pin - 1
}

pub fn devconfig(c: &mut configGenerator::BackendConfig) -> Result<(), Box<dyn Error>> {

    // Insert line into the config file
    let pattern = r"struct jailhouse_memory mem_regions\[\d+\];";

    let file_path = Path::new(WORKPATH).join("platform_info.toml");
    let templates_map = get_templates_map(); // Get all templates
    
    // Get minimum BDF from c.bdf (a cell without network has no PCI device)
    let bdf_used = if c.net == "none" {
        0
    } else {
        *c.bdf.iter().min().ok_or("no free ivshmem device (PCI BDF) for a cell with network")?
    };

    // Read device configurations from the platform TOML file
    let config: toml::Value = toml::from_str(&std::fs::read_to_string(&file_path)?)?;
    let devs = config.get("devices")
        .and_then(|dev| dev.get("devs"))
        .and_then(|devs| devs.as_array())
        .ok_or("Device list 'devs' not found in configuration")?;

    // Initialize num_pci_dev with zero
    let mut num_pci_dev = 0;
    let mut num_irq_chip = 0;

    // Fill the variable with the number of pci_devices inferred from the actually used templates
    for dev in devs {
        if let Some(dev_name) = dev.as_str() {
            match dev_name {
                "PCI_DEVICE_EMPTY_TEMPLATE" => {
                    // Skip these devices
                    continue;
                },
                "IRQ_CHIP_TEMPLATE" | "IRQ_CHIP_BOARD_TEMPLATE" => {
                    // Count this device as one
                    num_irq_chip += 1;
                },
                "PCI_DEVICE_TEMPLATE_WITH_DEMO" => {
                    // Count this device as two
                    num_pci_dev += 2;
                },
                _ => {
                    // Count all other devices as one
                    num_pci_dev += 1;
                },
            }
        }
    }

    // Check the value of c.net and set linetoinsert accordingly
    let linetoinsert = if c.net == "none" {
        "\tstruct jailhouse_irqchip irqchips[1];\n\tstruct jailhouse_pci_device pci_devices[0];"
    } else {    
        &format! ("\tstruct jailhouse_irqchip irqchips[{}];\n\tstruct jailhouse_pci_device pci_devices[{}];",num_irq_chip ,num_pci_dev)
    };

    // Compile a regular expression to match the pattern and insert the pci_devices
    let re = Regex::new(pattern)?;
    if let Some(pos) = re.find(&c.conf) {
        c.conf
            .insert_str(pos.end(), &format!("\n{}", linetoinsert));
    } else {
        return Err("\"pattern struct jailhouse_memory mem_regions not found".into());
    }

    // Placeholder insertions for each device in devs
    let mut result = String::new();
    for device in devs {
        if let Some(template_name) = device.as_str() {
            // Without network the cell has no PCI device (pci_devices[0] above):
            // leave the platform's ivshmem devices out.
            if c.net == "none" && template_name.starts_with("PCI_DEVICE") {
                continue;
            }
            // Get the template from the map
            let mut template = templates_map.get(template_name)
                .ok_or_else(|| format!("Unknown template: {}", template_name))?
                .to_string();

            // Substitute common fields
            template = template.replace("{ivshmem_bdf}", &bdf_used.to_string());
            // With vpci_irq_base the cell gets the INTx of its own ivshmem device,
            // as the hypervisor raises it: 32 + vpci_irq_base + pin - 1, the pin
            // rotated by the device number (pin = (device & 3) + 1); none without
            // network. ivshmem_pin of the platform file is the fallback.
            if let Some(base) = c.vpci_irq_base {
                if c.net == "none" {
                    template = template.replace("(1 << ({ivshmem_pin} - 128))", "0");
                } else {
                    let irq = ivshmem_intx(base, bdf_used as u32);
                    if !(128..160).contains(&irq) {
                        return Err(format!(
                            "ivshmem INTx {} (vpci_irq_base {}, device {}) is outside pins 128-159 of the irqchip template",
                            irq, base, bdf_used
                        )
                        .into());
                    }
                    template = template.replace("{ivshmem_pin}", &irq.to_string());
                }
            }
            // AGGIUSTARE PER NON PRENDERE ANCHE IL TEMPLATE DI DEVICE SE C.NET=NONE
            // Apply custom fields from TOML for the current device template
            if let Some(params) = config.get(template_name) {
                let params_str = params.as_table()
                    .ok_or_else(|| format!("Expected table for template: {}", template_name))?
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or("")))
                    .collect::<Vec<_>>()
                    .join(", ");
                
                // Generate the final device configuration using template and params
                result.push_str(&generate_mem_region(&template, &params_str));
            } else {
                // Append template with no custom parameters
                result.push_str(&template);
            }
        }
    }

    // Update the config with dynamically generated result
    c.conf.push_str(&result);

    Ok(())
}

// Replaces the placeholders in the template with the actual values from the `params` string
fn generate_mem_region(template: &str, params: &str) -> String {
    let mut filled_template = template.to_string();

    // If params are empty, skip the parameter replacement logic
    if params.is_empty() {
        return filled_template;
    }

    // Split the params string and replace each placeholder in the template
    for param in params.split(", ") {
        let mut key_value = param.split('=');

        let key = match key_value.next() {
            Some(k) => k.trim(),
            None => {
                eprintln!("Warning: Malformed parameter (missing key): {}", param);
                continue;
            }
        };

        let value = match key_value.next() {
            Some(v) => v.trim(),
            None => {
                eprintln!("Warning: Malformed parameter (missing value for key {}): {}", key, param);
                continue;
            }
        };

        filled_template = filled_template.replace(&format!("{{{}}}", key), value);
    }

    filled_template
}

#[cfg(test)]
mod tests {
    use super::ivshmem_intx;

    #[test]
    fn intx_of_the_kria_devices() {
        // kria_platform_info.toml: vpci_irq_base 108
        assert_eq!(ivshmem_intx(108, 1), 141);
        assert_eq!(ivshmem_intx(108, 2), 142);
        // the root cell's own (vpci_irq_base 136 - 32)
        assert_eq!(ivshmem_intx(104, 1), 137);
        assert_eq!(ivshmem_intx(104, 2), 138);
    }
}
