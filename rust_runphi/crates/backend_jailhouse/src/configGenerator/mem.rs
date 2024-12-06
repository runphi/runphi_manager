//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (f.boccola@studenti.unina.it)
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
//use f2b;

//use crate::configGenerator::templates::{RAM_TEMPLATE, UART_TEMPLATE};
use crate::configGenerator::templates::*;

const WORKPATH: &str = "/usr/share/runPHI";
//const PCI_IVSHMEM_ID_FILE: &str = "pci_ivshmem_id.txt";

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



// Generates the final configuration string by replacing the placeholders with the correct values
fn generate_config(
    config: &Value,
    mem_request_hex: &str,
    skip_ivshmem: bool,
    base_address: &str,
    ram0_phys_start: &str,
    ram_phys_start: &str,
) -> Result<String, String> {
    let mut result = String::new();
    let templates_map = get_templates_map(); // Retrieve the map of templates

    // Safely check if the mem_regions exist in the config
    if let Some(mem_regions) = config.get("mem_regions") {
        if let Some(regions) = mem_regions.get("regions").and_then(|r| r.as_array()) {
            for region in regions {
                if let Some(template_name) = region.as_str() {
                    // Skip IVSHMEM templates if `skip_ivshmem` is true
                    if skip_ivshmem
                        && (template_name == "IVSHMEM_DEMO_TEMPLATE" || template_name == "IVSHMEM_TEMPLATE")
                    {
                        continue;
                    }

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
                            .replace("{size}", mem_request_hex);
                    }
                    if template_name == "IVSHMEM_TEMPLATE" {
                        template = template.replace("{address}", base_address);
                    }

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
                } else {
                    return Err("Region is not a string".to_string());
                }
            }
        } else {
            return Err("mem_regions.regions is not an array".to_string());
        }
    } else {
        return Err("mem_regions key missing in config".to_string());
    }

    Ok(result)
}


pub fn memconfig(
    c: &mut configGenerator::Backendconfig,
    mem_request_hex: &str,
) -> Result<(), Box<dyn Error>> {
    let file_path = Path::new(WORKPATH).join(format!("platform-info.toml"));

    // Insert line into the config file
    let pattern = r"__u64 rcpus\[\d*\];";
    let num_regions = match count_mem_regions(&file_path) {
    Ok(count) => count,
    Err(e) => {
        eprintln!("Error counting memory regions: {}", e);
        return Err(e.into()); // Propagate the error
        }
    };
    // Use format! to include the variable
    let linetoinsert = format!("\tstruct jailhouse_memory mem_regions[{}];", num_regions); 

    // Compile a regular expression to match the pattern and insert the cpus
    let re = Regex::new(&pattern)?;
    if let Some(pos) = re.find(&c.conf) {
        c.conf
            .insert_str(pos.end(), &format!("\n{}\n", linetoinsert));
    } else {
        return Err("\"struct jailhouse_cell_desc cell\" not found".into());
    }

    match load_config(&file_path) {
        Ok(config) => {
            // Check if we need to skip IVSHMEM templates based on `c.net`
            let skip_ivshmem = c.net == "none";

            // Calculate the base address based on the lowest value in c.bdf
            if let Some(&min_bdf) = c.bdf.iter().filter(|&&b| b > 0).min() {
                // Retrieve the base address from IVSHMEM_TEMPLATE in the TOML configuration
                let base_address = config
                .get("IVSHMEM_TEMPLATE")
                .and_then(|section| section.get("address"))
                .and_then(|addr| addr.as_str())
                .ok_or("IVSHMEM_TEMPLATE address not found in configuration")?;

                // Parse the base address from hex string to u64
                let base_address = u64::from_str_radix(base_address.trim_start_matches("0x"), 16)? + ((min_bdf as u64 - 1) * 0x100000);

                let address_hex = format!("0x{:x}", base_address);


                // Check if RAM0_TEMPLATE is present in the regions list
                let has_ram0_template = config.get("mem_regions")
                .and_then(|mem_regions| mem_regions.get("regions"))
                .and_then(|regions| regions.as_array())
                .map_or(false, |regions| regions.iter().any(|r| r.as_str() == Some("RAM0_TEMPLATE")));
                
                // Calculate required memory size in hexadecimal, adding 0x10000 for RAM0_TEMPLATE if present
                let mem_request_size = u64::from_str_radix(&mem_request_hex.trim_start_matches("0x"), 16)?;
                let required_size = if has_ram0_template {
                    mem_request_size + 0x10000
                } else {
                    mem_request_size
                };

                // Find a suitable segment in `c.segments`
                if let Some((segment_index, chosen_segment)) = c.segments.iter().enumerate()
                    .find_map(|(i, seg)| {
                        let parts: Vec<&str> = seg.split(", ").collect();
                        if parts.len() != 2 { return None; }  // Segment is not in the correct format

                        // Parse start and end of segment
                        let start = u64::from_str_radix(parts[0].trim_start_matches("0x"), 16).ok()?;
                        let end = u64::from_str_radix(parts[1].trim_start_matches("0x"), 16).ok()?;
                        
                        // Check if the segment is large enough
                        if end - start >= required_size {
                            Some((i, (start, end)))
                        } else {
                            None
                        }
                    })
                {
                    // Update the chosen segment's start address and push remaining free space back to `c.segments`
                    let new_start = chosen_segment.0 + required_size;
                    c.segments[segment_index] = format!("0x{:x}, 0x{:x}", new_start, chosen_segment.1);

                    // Set the physical start addresses
                    let ram0_phys_start = format!("0x{:x}", chosen_segment.0);
                    let ram_phys_start = if has_ram0_template {
                        format!("0x{:x}", chosen_segment.0 + 0x10000)
                    } else {
                        ram0_phys_start.clone()
                    };

                    match generate_config(&config, mem_request_hex, skip_ivshmem, &address_hex, &ram0_phys_start, &ram_phys_start) {
                        Ok(config_string) => {
                            // Wrap the configuration in the .mem_regions jailhouse field
                            let wrapped_config = format!(
                                ".mem_regions = {{\n{}\n}},\n",
                                config_string
                            );
                            // Append the generated configuration to c.conf
                            c.conf.push_str(&wrapped_config);
                            
                        }
                        Err(e) => eprintln!("Error generating config: {}", e),
                    }
                } else {
                    eprintln!("Error: No suitable segment found in `c.segments` for required memory size.");
                }
            } else {
                eprintln!("Error: No valid bdf values found in `c.bdf`");
            }
        }
        Err(e) => eprintln!("Error loading config: {}", e),
    }

    Ok(())
}

fn count_mem_regions<P: AsRef<Path>>(file_path: P) -> Result<usize, Box<dyn std::error::Error>> {
    // Read the content of the file
    let content = fs::read_to_string(file_path)?;
    
    // Parse the content as TOML
    let toml_value: Value = content.parse()?;
    
    // Access the "mem_regions" table
    let mem_regions = toml_value
        .get("mem_regions")
        .and_then(|table| table.get("regions"))
        .and_then(|regions| regions.as_array())
        .ok_or("Failed to read 'mem_regions.regions' as an array")?;

    // Count the regions with specific rules
    let mut count = 0;
    for region in mem_regions {
        if let Some(region_name) = region.as_str() {
            match region_name {
                "IVSHMEM_DEMO_TEMPLATE" => count += 5,
                "IVSHMEM_TEMPLATE" => count += 4,
                _ => count += 1,
            }
        }
    }

    Ok(count)
}
