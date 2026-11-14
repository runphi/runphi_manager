//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (francesco.boccola@unina.it)
//*********************************************

use regex::Regex;
use std::error::Error;
//use std::fs::OpenOptions; //DEBUG
//use std::fs::File;
use std::fs;
use toml::Value;
//use std::str::FromStr;
//use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
//use std::collections::HashMap;

use crate::configGenerator;
use crate::configGenerator::rpu;
//use f2b;

//use crate::configGenerator::templates::{RAM_TEMPLATE, UART_TEMPLATE};
use crate::configGenerator::templates::*;

use f2b::paths::WORKPATH;

// Loads the configuration from the specified file.
fn load_config(file_path: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(file_path)?;
    let config: Value = toml::from_str(&contents)?;
    Ok(config)
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



// The memory region templates of the platform that this cell gets: without
// network no ivshmem regions; on a platform with R5 cores described in
// platform_info.toml ([rcpu.<id>]), the TCM regions only for an R5 cell.
fn cell_regions(config: &Value, skip_ivshmem: bool, skip_tcm: bool) -> Result<Vec<String>, String> {
    let regions = config
        .get("mem_regions")
        .ok_or("mem_regions key missing in config")?
        .get("regions")
        .and_then(|r| r.as_array())
        .ok_or("mem_regions.regions is not an array")?;
    let mut names = Vec::new();
    for region in regions {
        let name = region.as_str().ok_or("Region is not a string")?;
        if skip_ivshmem && (name == "IVSHMEM_DEMO_TEMPLATE" || name == "IVSHMEM_TEMPLATE") {
            continue;
        }
        if skip_tcm && (name == "TCMA_TEMPLATE" || name == "TCMB_TEMPLATE") {
            continue;
        }
        names.push(name.to_string());
    }
    Ok(names)
}

// Number of jailhouse_memory entries of the regions (the ivshmem templates
// stand for several)
fn count_mem_regions(regions: &[String]) -> usize {
    regions
        .iter()
        .map(|name| match name.as_str() {
            "IVSHMEM_DEMO_TEMPLATE" => 5,
            "IVSHMEM_TEMPLATE" => 4,
            _ => 1,
        })
        .sum()
}

fn hex(v: u64) -> String {
    format!("0x{:x}", v)
}

// Generates the final configuration string by replacing the placeholders with the correct values
fn generate_config(
    config: &Value,
    regions: &[String],
    ram_size_hex: &str,
    base_address: &str,
    ram0_phys_start: &str,
    ram_phys_start: &str,
    r5: Option<&rpu::RcpuInfo>,
) -> Result<String, String> {
    let mut result = String::new();
    let templates_map = get_templates_map(); // Retrieve the map of templates

    for template_name in regions {
        let template_name = template_name.as_str();
        // Look up the template in the templates_map
        let mut template = match templates_map.get(template_name) {
            Some(&template) => template.to_string(),
            None => return Err(format!("Unknown template: {}", template_name)),
        };

        // Apply specific replacements for RAM and IVSHMEM templates
        if template_name == "RAM0_TEMPLATE" {
            template = template.replace("{phys_start}", ram0_phys_start);
        }
        if template_name == "RAM_TEMPLATE" {
            template = template
                .replace("{phys_start}", ram_phys_start)
                .replace("{size}", ram_size_hex);
        }
        if template_name == "IVSHMEM_TEMPLATE" {
            template = template.replace("{address}", base_address);
        }
        // An R5 cell's TCMs are those of its core
        if let Some(r) = r5 {
            let tcm = match template_name {
                "TCMA_TEMPLATE" => Some(r.tcma),
                "TCMB_TEMPLATE" => Some(r.tcmb),
                _ => None,
            };
            if let Some(tcm) = tcm {
                template = template
                    .replace("TCM 0-", &format!("TCM {}-", r.id))
                    .replace("{phys_start}", &hex(tcm))
                    .replace("{virt_start}", &hex(tcm))
                    .replace("{size}", &hex(r.tcm_size));
            }
        }

        // Auto indent templates
        template = template
            .lines()
            .map(|line| format!("\t\t{}", line))
            .collect::<Vec<String>>()
            .join("\n");

        // Process additional parameters from the config
        if let Some(params) = config.get(template_name) {
            let params_str = params.as_table()
                .ok_or_else(|| format!("Expected table for template: {}", template_name))?
                .iter()
                .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or("")))
                .collect::<Vec<_>>()
                .join(", ");
            result.push_str(&generate_mem_region(&template, &params_str));
        } else {
            // Append templates with no params directly
            result.push_str(&template);
        }
    }

    Ok(result)
}

fn parse_segment(seg: &str) -> Option<(u64, u64)> {
    let (start, end) = seg.split_once(", ")?;
    Some((
        u64::from_str_radix(start.trim().trim_start_matches("0x"), 16).ok()?,
        u64::from_str_radix(end.trim().trim_start_matches("0x"), 16).ok()?,
    ))
}

// The memory regions of the cell. An A53 cell gets `mem_request_hex` bytes of
// the free segments of state.toml (the pool, recorded in c.memory); an R5 cell
// of a platform with [rcpu.<id>] sections gets its core's DDR region and TCMs,
// where its program is linked, and nothing from the pool.
pub fn memconfig(
    c: &mut configGenerator::BackendConfig,
    mem_request_hex: &str,
) -> Result<(), Box<dyn Error>> {
    let file_path = Path::new(WORKPATH).join("platform_info.toml");
    let config = load_config(&file_path)?;

    let skip_ivshmem = c.net == "none";
    let r5 = if c.rcpu_layout { c.rcpu.clone() } else { None };
    let skip_tcm = c.rcpu_layout && r5.is_none();
    let regions = cell_regions(&config, skip_ivshmem, skip_tcm)?;

    // Declare the regions after the cpus (or rcpus) set
    let linetoinsert = format!("\tstruct jailhouse_memory mem_regions[{}];", count_mem_regions(&regions));
    let re1 = Regex::new(r"__u64 rcpus\[\d*\];")?;
    let re2 = Regex::new(r"__u64 cpus\[\d*\];")?;
    let pos = if let Some(m) = re1.find(&c.conf) {
        m
    } else if let Some(m) = re2.find(&c.conf) {
        m
    } else {
        return Err("Neither pattern __u64 rcpus nor __u64 cpus found".into());
    };
    c.conf.insert_str(pos.end(), &format!("\n{}\n", linetoinsert));

    // The shared memory of the cell's ivshmem device, one per BDF
    let address_hex = if skip_ivshmem {
        String::new()
    } else {
        let min_bdf = *c
            .bdf
            .iter()
            .filter(|&&b| b > 0)
            .min()
            .ok_or("no free ivshmem device (PCI BDF) for a cell with network")?;
        let base_address = config
            .get("IVSHMEM_TEMPLATE")
            .and_then(|section| section.get("address"))
            .and_then(|addr| addr.as_str())
            .ok_or("IVSHMEM_TEMPLATE address not found in configuration")?;
        let base_address = u64::from_str_radix(base_address.trim_start_matches("0x"), 16)?
            + ((min_bdf as u64 - 1) * 0x100000);
        hex(base_address)
    };

    let has_ram0_template = regions.iter().any(|r| r == "RAM0_TEMPLATE");

    let (ram0_phys_start, ram_phys_start, ram_size_hex) = if let Some(r) = &r5 {
        c.memory = None;
        (hex(r.ddr_start), hex(r.ddr_start), hex(r.ddr_size))
    } else {
        // Calculate required memory size, adding 0x10000 for RAM0_TEMPLATE if present
        let mem_request_size = u64::from_str_radix(mem_request_hex.trim_start_matches("0x"), 16)?;
        let required_size = if has_ram0_template {
            mem_request_size + 0x10000
        } else {
            mem_request_size
        };

        // First free segment that is large enough
        let (segment_index, (start, end)) = c
            .segments
            .iter()
            .enumerate()
            .find_map(|(i, seg)| {
                let (start, end) = parse_segment(seg)?;
                (end.saturating_sub(start) >= required_size).then_some((i, (start, end)))
            })
            .ok_or_else(|| {
                format!(
                    "not enough free memory for the cell: need 0x{:x} bytes, free segments {:?}",
                    required_size, c.segments
                )
            })?;

        // Take it from the segment (and drop the segment if nothing is left)
        let new_start = start + required_size;
        if new_start < end {
            c.segments[segment_index] = format!("0x{:x}, 0x{:x}", new_start, end);
        } else {
            c.segments.remove(segment_index);
        }
        c.memory = Some((start, new_start));

        let ram_phys_start = if has_ram0_template { start + 0x10000 } else { start };
        (hex(start), hex(ram_phys_start), mem_request_hex.to_string())
    };

    let config_string = generate_config(
        &config,
        &regions,
        &ram_size_hex,
        &address_hex,
        &ram0_phys_start,
        &ram_phys_start,
        r5.as_ref(),
    )?;
    // Wrap the configuration in the .mem_regions jailhouse field
    c.conf
        .push_str(&format!("\n\t.mem_regions = {{{}\n\t}},\n", config_string));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // kria_platform_info.toml's TCM sections, which name core 0's TCMs
    const PLATFORM: &str = r#"
[mem_regions]
regions = ["UART_TEMPLATE", "TCMA_TEMPLATE", "TCMB_TEMPLATE", "RAM_TEMPLATE", "COMM_REGION_TEMPLATE"]
[UART_TEMPLATE]
phys_start = "0xff010000"
virt_start = "0xff010000"
size = "0x1000"
[TCMA_TEMPLATE]
phys_start = "0xffe00000"
virt_start = "0xffe00000"
size = "0x00010000"
[TCMB_TEMPLATE]
phys_start = "0xffe20000"
virt_start = "0xffe20000"
size = "0x00010000"
[RAM_TEMPLATE]
phys_start = "0x4f000000"
virt_start = "0"
size = "0x8000000"
"#;

    fn core1() -> rpu::RcpuInfo {
        rpu::RcpuInfo { id: 1, tcma: 0xffe90000, tcmb: 0xffeb0000, tcm_size: 0x10000, ddr_start: 0x3ad00000, ddr_size: 0x4000000 }
    }

    #[test]
    fn r5_core1_gets_its_own_tcm_and_ddr() {
        let config: Value = PLATFORM.parse().unwrap();
        let r = core1();
        let regions = cell_regions(&config, true, false).unwrap();
        let out = generate_config(&config, &regions, &hex(r.ddr_size), "", &hex(r.ddr_start), &hex(r.ddr_start), Some(&r)).unwrap();
        assert!(out.contains("TCM 1-A") && out.contains("TCM 1-B"), "{}", out);
        assert!(out.contains(".phys_start = 0xffe90000") && out.contains(".phys_start = 0xffeb0000"), "{}", out);
        assert!(!out.contains("0xffe00000") && !out.contains("0xffe20000"), "core 0's TCM in a core 1 cell: {}", out);
        assert!(out.contains(".phys_start = 0x3ad00000") && out.contains(".size = 0x4000000"), "{}", out);
    }

    #[test]
    fn a53_cell_gets_no_tcm() {
        let config: Value = PLATFORM.parse().unwrap();
        let regions = cell_regions(&config, true, true).unwrap();
        assert!(!regions.iter().any(|r| r.starts_with("TCM")));
        assert_eq!(count_mem_regions(&regions), 3);
        let out = generate_config(&config, &regions, "0x4000000", "", "0x4f000000", "0x4f000000", None).unwrap();
        assert!(!out.contains("TCM") && out.contains(".phys_start = 0x4f000000"), "{}", out);
    }
}
