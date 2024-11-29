use regex::Regex;
use std::error::Error;
use std::fs::{self};
use std::path::Path;
use toml::Value;


use crate::configGenerator;
use f2b;

const WORKPATH: &str = "/usr/share/runPHI/";
const FPGA_FILE: &str =  "fpga_info.toml";


pub fn stream_id_config(c: &mut configGenerator::Backendconfig) -> Result<(), Box<dyn Error>>  {

    let num_stream_ids = if c.used_fpga_regions.is_empty() { 0 } else { 1 };
    // insert FPGA master stream id for our soft core. temporary
    let pattern = r"struct jailhouse_pci_device pci_devices\[\d*\];";
    //let pattern = r"struct jailhouse_memory mem_regions\[\d+\];";
    let re = Regex::new(&pattern)?;

    let linetoinsert = format!("\tunion jailhouse_stream_id stream_ids[{}];",num_stream_ids); //for now 1
    if let Some(pos) = re.find(&c.conf) { //try to place after rcpus
        c.conf
            .insert_str(pos.end(), &format!("\n{}", linetoinsert));
    }else{
        return Err("\"struct jailhouse_memory mem_regions\" not found".into());
    }

    let stream_id = match num_stream_ids {
        1 =>r#"
        .stream_ids = {
                {
                    .mmu500.id = 0x1280,
                    .mmu500.mask_out = 0x3f, // Mask out bits 0..5
                },
            },
        "#,
        _ => r#"
        .stream_ids = {
            },
        "#,
    };
    c.conf
        .push_str(&stream_id);
   
    return Ok(());

}
/**
if the requested regions are available, remove them from free and give them to assigned
return number of regions assigned

if they are not available, do not modify free or assigned and return 0
*/
pub fn regions_available(free: &mut Vec<i8>, requested: &str, assigned: &mut Vec<i8>) -> usize {
    let regions: Vec<i8> = requested.split('-').map(|s| s.parse::<i8>().unwrap()).collect();   
    for r in &regions {
        //check if free contains region
        if !free.contains(&r){
            return 0;
        }
    } 
    for r in &regions {
        assigned.push(*r);
    }
    //remove from free
    free.retain(|x| !assigned.contains(x));
    return regions.len();
}

pub fn fpgaconf(
    c: &mut configGenerator::Backendconfig,
    config: &mut f2b::ImageConfig
) -> Result<(), Box<dyn Error>> {

    // Check availability
    let mut free_fpga_regions = c.fpga_regions.clone();

    let mut regionsassigned: Vec<i8> = Vec::new(); 
    let mut fpga_bitmask: u32 = 0;

    //read fpga-info.toml
    let config_content = fs::read_to_string(Path::new(WORKPATH).join(FPGA_FILE))?;

    // Parse the content as TOML
    let parsed_toml: Value = config_content.parse::<Value>()?;
    let hardware_designs = parsed_toml
        .get("hardware_designs")
        .and_then(|table| table.get("designs"))
        .and_then(|designs| designs.as_array())
        .ok_or("Failed to read 'hardware_designs.designs' as an array")?;
    

    'accelerators: for accelerator in &config.accelerators {
        // for each accelerator
        if accelerator.bitstream.is_empty(){  // if we were given core name, and not bstream directly
            if let Some(_found_design) = hardware_designs.iter().find(|&design| design.as_str() == Some(&accelerator.core)){  
                let versions_toml = parsed_toml
                .get(&accelerator.core)
                .and_then(|design| design.get("versions"))
                .and_then(|v| v.as_array())
                .ok_or("Failed to read 'versions' for the target design")?;

                let versions: Vec<String> = versions_toml
                .iter()
                .filter_map(|val| val.as_str().map(|s| s.to_string()))
                .collect();
    
                for version in &versions{

                    let name = parsed_toml
                    .get(version)
                    .and_then(|v| v.get("name"))
                    .and_then(|n| n.as_str())
                    .ok_or("Failed to convert 'name' to string")?;

                    let region = parsed_toml
                    .get(version)
                    .and_then(|v| v.get("region"))
                    .and_then(|r| r.as_integer().or_else(|| r.as_str().and_then(|s| s.parse::<i64>().ok())))
                    .and_then(|num| num.try_into().ok())
                    .ok_or("Failed to read 'region'")?; 
                    
                    if !free_fpga_regions.contains(&region){
                        continue;
                    }
                    free_fpga_regions.retain(|x| x != &region);
                    regionsassigned.push(region);

                    config.bitstreams.push(name.to_string());
                    config.regions.push(region);
                    
                    // memory
                    if let Some(phys_start) = parsed_toml
                    .get(version)
                    .and_then(|v| v.get("phys_start"))
                    .and_then(|value| value.as_str())
                    {
                        if let Some(mem_size) = parsed_toml
                        .get(version)
                        .and_then(|v| v.get("size"))
                        .and_then(|value| value.as_str())
                        {
                            let virt_start = parsed_toml
                                .get(version)
                                .and_then(|v| v.get("virt_start"))
                                .and_then(|value| value.as_str())
                                .map(|address| address.to_string())
                                .unwrap_or_else(|| "0".to_string());
                            c.soft_core_mem.push_str(format!("{}; {}; {}",phys_start, virt_start,mem_size).as_str());    
                        }else{
                            return Err("Unable to program FPGA: memory size for accelerator not present".to_owned().into());
                        }
                    }
                    continue 'accelerators;
                }
            
                return Err("Unable to program FPGA: region unavailable".to_owned().into());
            
            }else{
                return Err("Unable to program FPGA: bitstream not found".to_owned().into());
            }
        } 
        else{ // we were given bitstream inside the path
            let file_name = Path::new(&accelerator.bitstream) 
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap();
            
            let region = &accelerator.region.parse::<i8>()?; //??
            if free_fpga_regions.contains(region) {
                
                free_fpga_regions.retain(|x| x != region);
                regionsassigned.push(*region);
                config.bitstreams.push(file_name.to_string());
                config.regions.push((*region).into());
                
                // if any accelerator requires a RAM, insert here
                // FORMAT: phys_address; starting_address; size
                // ASSUMING FOR NOW JUST 1
                for accelerator in &config.accelerators{
                    if !accelerator.starting_phys_address.is_empty(){
                        let linetoinsert = if accelerator.acc_starting_vaddress.is_empty() {
                            &format!("{}; 0; {}",accelerator.starting_phys_address, accelerator.mem_size)
                        }else{
                            &format!("{}; {}; {}",accelerator.starting_phys_address, accelerator.acc_starting_vaddress, accelerator.mem_size)
                        };
                        c.soft_core_mem.push_str(linetoinsert);
                        break;
                    }
                }
            } 
            else{
                return Err("Unable to program FPGA: not enough space available or bitstream not found".to_owned().into());
            }
        } 
    }
    
    c.used_fpga_regions = regionsassigned.clone();
    c.fpga_regions = free_fpga_regions;

    
     for &region in &regionsassigned {
        fpga_bitmask |= 1 << region;
    } 

    let hex_str = format!("{:x}", fpga_bitmask);

    c.conf
    .push_str(&format!("\n\t.fpga_regions = {{\n\t\t0x{},\n\t}},\n", hex_str)); //for cpus in 0x

    return Ok(());
}
