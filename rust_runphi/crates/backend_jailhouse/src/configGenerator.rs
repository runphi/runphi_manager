//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (f.boccola@studenti.unina.it)
//*********************************************

use regex::Regex;
use std::error::Error;
//use std::fs::{File, self, OpenOptions};
use std::fs;
use std::io::Write;
//use std::time::Instant;   //TIME CLOCK MONOTONIC
//use std::process::Command;
use std::str;
use toml::{Value, map::Map};
use std::path::{Path, PathBuf};

use f2b;
pub mod boot;
pub mod communication;
pub mod cpu;
pub mod device;
pub mod mem;
pub mod network;
pub mod templates;
pub mod rpu;
pub mod fpga;
use crate::configGenerator::templates::*;

const WORKPATH: &str = "/usr/share/runPHI";
//const RUNDIR: &str = "/run/runPHI";
const STATEFILE: &str = "state.toml";
const CONFIG_FILE: &str = "platform-info.toml";


// This structure holds all the information related to the configuration of the partitioned container
// There is the configuration file, the configuration string, and needed variables for resources,
// like cpus, memory addresses, devices, and in general all the output of the configGeneration phase
#[derive(Debug)]
pub struct Backendconfig {
    pub conf: String,
    pub cpus: u8,
    pub conffile: String,
    pub net: String,
    pub rpu_req: bool,
    pub segments: Vec<String>,   
    pub bdf: Vec<i8>,
    pub rcpus: Vec<i8>,
    pub used_rcpus: Vec<i8>,
    pub fpga_regions: Vec<i8>,
    pub used_fpga_regions: Vec<i8>,
    pub soft_core_mem: String,
}

impl Backendconfig {
    // Constructor function
    pub fn new() -> Self {
        Self {
            conf: String::new(),
            cpus: 0,
            conffile: String::new(),
            net: String::new(),
            rpu_req: false,
            segments: Vec::new(),  
            bdf: Vec::new(),
            rcpus: Vec::new(),
            used_rcpus: Vec::new(),
            fpga_regions: Vec::new(),
            used_fpga_regions: Vec::new(),
            soft_core_mem: String::new(),
        }
    }
}

//TODO: error handling across this function is a box of shit, handle it
pub fn config_generate(fc: &f2b::FrontendConfig) -> Result<Box<f2b::ImageConfig>, Box<dyn Error>> {
    logging::log_message(logging::Level::Debug, format!("Starting config generator for id {}", &fc.containerid).as_str());

    let mut c = Backendconfig::new();
    c.conffile = format!("{}/config{}.conf", fc.crundir, fc.containerid);

    // parsing configuration variables from the file
    //THIS IS THE ACCESS TO JSON.CONFIG FROM DOCKER
    logging::log_message(logging::Level::Debug, format!("Reading the config.json inside the container for id {}", &fc.containerid).as_str());
    let mut config = Box::new(f2b::ImageConfig::get_from_file(&fc.mountpoint));
    //Clone the value of config.net (from the internal .json) to c.net
    c.net = config.net.clone();

    // Do we require rpus?
    c.rpu_req = config.rpu_req;

    // Read the state ogf the machine from the state.toml file (in particular free memory and free bdfs)
    let (segments, bdf, rcpus, fpga_regions) = retrieve_state()?;
    // Update the struct
    c.segments = segments;
    c.bdf = bdf;
    c.rcpus = rcpus;
    c.fpga_regions = fpga_regions;



    
    //c.preamble = preamble;

    logging::log_message(logging::Level::Debug, format!("Config helper start for id {}", &fc.containerid).as_str());
    let _ = confighelperstart(fc, &mut c, &config);

    // This region of code could be extended with code to retrieve other specific Docker's flags which set CPU limitations
    // cpus where allow guest execution set by Docker's flag 'cpuset-cpus'
    // If flag is not set, let's go for 1. It will be overwritten by quota and period if they are defined
    let _cpu_set = fc.jsonconfig["linux"]["resources"]["cpu"]["cpus"]
        .as_f64()
        .unwrap_or(1.0);

    //Through Docker's flag "cpus=0.0000" user requires an amount cpus usage as percentage
    //That percentage will be expressed in form of quota-period ratio (EG: cpus=2.00 means values:
    // quota=20000 and period=10000 --> cpus=2)
    // Extract period and quota as floats
    // Set default value to 1.0 if not present or not a float
    let period = fc.jsonconfig["linux"]["resources"]["cpu"]["period"]
        .as_f64()
        .unwrap_or(10000.0);

    // Set default value to 1.0 if not present or not a float
    let quota = fc.jsonconfig["linux"]["resources"]["cpu"]["quota"]
        .as_f64()
        .unwrap_or(10000.0);

    logging::log_message(logging::Level::Debug, format!("_cpu_set={}, period={}, quota={}", _cpu_set,period,quota).as_str());

    // cpus is a floating point number
    // If the backend does not support fractional allots, that's a backend matter
    let mut cpus: f64 = quota / period;

    /*
     Here can be implemented: hypervisor agnostic real-time schedulability tests, etc.
    */

    logging::log_message(logging::Level::Debug, format!("Configuring CPU for id {}", &fc.containerid).as_str());
    

    //If rpu_req is true we are requesting RPUs and not CPUs
    //c.rpu_req=true; //For testing purposes
    if c.rpu_req{
        let rpus=cpus;
        cpus=0.0;
        let _ = cpu::cpuconf(fc, &mut c, &quota, &period, &cpus);
        let _ = rpu::rpuconf(&mut c, &rpus);
    } else { 
        //let rpus=0.0;
        let _ = cpu::cpuconf(fc, &mut c, &quota, &period, &cpus);
        //let _ = rpu::rpuconf(&mut c, &rpus);
    }
    
    
    //logging::log_message(logging::Level::Debug, format!("\nconfiguration after cpuconf is  {}", c.conf).as_str());

    //request fpga_regions
    let _ = fpga::fpgaconf(&mut c, &mut config);
    //logging::log_message(logging::Level::Debug, format!("\nconfiguration after fpgaconf is  {}", c.conf).as_str());
   
    
    //temporary; if bitstreams contains pico32, its a riscv core
    //let riscv_core = config.bitstreams.iter().any(|s| s.contains("pico32"));

    //This region of code could be extended through code to retrieve other specific Docker's flags which set MEM limitations
    // Extract values from the JSON structure
    //In the json structure only limit is created by kubernetes memory reservation doesn't exist so I'll comment it
    /* let _mem_res = fc.jsonconfig["linux"]["resources"]["memory"]["reservation"] //Domain memory in MB, (--memory-reservation="")
    .as_u64() // Assuming memory values are in unsigned integers
    .unwrap_or(512); // Set default value to 512 MB if the value is missing */

    let mem_request = fc.jsonconfig["linux"]["resources"]["memory"]["limit"] //Maximum domain memory in MB, (-m, --memory="")
        .as_u64() // Assuming memory values are in unsigned integers
        .unwrap_or(67_108_864); // Set default value to 64 MiB if the value is missing

    //Convert the value parsed to a hexadecimal String
    let mem_request_hex = format!("0x{:x}", mem_request);

    //Save the value of segments 
    let segments_before=c.segments.clone();
    //Save the value of the bdf we use
    let bdf_used = if c.net != "none" {
        c.bdf.iter().min().cloned()
    } else {
        None
    };

    //Pass everything to memconfig
    logging::log_message(logging::Level::Debug, format!("Configuring memory for id {}", &fc.containerid).as_str());
    let _ = mem::memconfig(&mut c, &mem_request_hex); 

    logging::log_message(logging::Level::Debug, format!("Configuring Device for id {}", &fc.containerid).as_str());
    let _ = device::devconfig(&mut c);

    //temporarily insert stream ids if you have fpga?
    logging::log_message(logging::Level::Debug, format!("Inserting FPGA stream ids for id {}", &fc.containerid).as_str());
    let _ =fpga::stream_id_config(&mut c);

    let _ = boot::bootconfbackend(fc, &mut config);

    //TODO: call net config here (take net memory areas from memory)

    // Guest console is allocated when -t flag is provided
    // useful for Hypervisor like XEN or BAO which give the possibility
    // to start Guest with fully fledged OS
    // Jailhouse gives only the option to start non-root linux cell but the user can connect to it
    // only through ssh

    // If -t flag was specified, call COMMUNICATION backend for further processing
    // E.G. allocate terminal or ssh shell

    if !fc.guestconsole.is_empty() {
        let mut file = fs::File::create(format!("{}/console", fc.crundir))
            .expect("Failed to create console file");
        writeln!(file, "{}", fc.guestconsole).expect("Failed to write console file");
    }

    //let _ = communication::communicationconfig(&mut c); //communication è stato incluso direttamente nel preamble

    // Call save_state and log the result
    match save_state(
        &fc.containerid,
        &c.segments,
        &segments_before,
        &c.rcpus,
        &c.fpga_regions,
        bdf_used,
        &c.used_rcpus,
        &c.used_fpga_regions
    ) {
        Ok(_) => logging::log_message(logging::Level::Debug, format!("State saved successfully for id {}", &fc.containerid).as_str()),
        Err(_e) => logging::log_message(logging::Level::Debug, format!("Failed to save state for id {}", &fc.containerid).as_str()),
    }

    let _ = confighelperend(fc, &mut c, &config);

    //logging::log_message(logging::Level::Debug, format!("Finishing configuration for id {}", &fc.containerid).as_str());
    //logging::log_message(logging::Level::Debug, format!("\nactual configuration is  {}", c.conf).as_str());
    
    return Ok(config);
}

fn confighelperstart(
    fc: &f2b::FrontendConfig,
    c: &mut Backendconfig,
    ic: &f2b::ImageConfig,
) -> Result<(), Box<dyn Error>> {
    // Write the conf file preamble

    if ic.os_var == "zephyr" {

        logging::log_message(logging::Level::Debug, format!("Starting helper start for id {}", &fc.containerid).as_str());
        // Construct the full path to the TOML file
        let config_path = Path::new(WORKPATH).join(CONFIG_FILE);

        // Read the contents of the TOML file
        let config_content = fs::read_to_string(config_path)?;

        // Parse the content as TOML
        let parsed_toml: Value = config_content.parse::<Value>()?;

        logging::log_message(logging::Level::Debug, format!("Retrieving preamble for id {}", &fc.containerid).as_str());
        // Retrieve the `preamble` value under `[jailhouse_preample]`
        if let Some(preamble) = parsed_toml
            .get("jailhouse_preample")
            .and_then(|section| section.get("preamble"))
            .and_then(|p| p.as_str())
        {
            // Choose the appropriate template
            //logging::log_message(logging::Level::Debug, format!("Choosing template for id {}", &fc.containerid).as_str());
            let selected_template = match preamble {
                "QEMU_PREAMBLE" => QEMU_PREAMBLE_TEMPLATE,
                "ULTRASCALE_PREAMBLE" => ULTRASCALE_PREAMBLE_TEMPLATE,
                _ => return Err(format!("Unexpected preamble value: {}", preamble).into()),
            };

            // Replace `{containerid}` placeholder with `fc.containerid`
            let filled_template = selected_template.replace("{containerid}", &fc.containerid);
            //logging::log_message(logging::Level::Debug, format!("Replacement in template for id {}", &fc.containerid).as_str());

            // Append the filled template to `c.conf`
            c.conf.push_str(&filled_template);
            logging::log_message(logging::Level::Debug, format!("Conf preamble created for id {}", &fc.containerid).as_str());
        } else {
            return Err("Field 'preamble' not found in [jailhouse_preample]".into());
        }

    } else if ic.os_var == "linux" {
        c.conf = format!(
            "#include \"cell.h\"
    struct {{
        struct jailhouse_cell_desc cell;
    }} __attribute__((packed)) config = {{
        .cell = {{
            .signature = JAILHOUSE_CELL_DESC_SIGNATURE,
            .revision = JAILHOUSE_CONFIG_REVISION,
            .name = \"{}\",
            .flags = JAILHOUSE_CELL_PASSIVE_COMMREG |
                JAILHOUSE_CELL_VIRTUAL_CONSOLE_PERMITTED,
    
            .cpu_set_size = sizeof(config.cpus),
            .num_memory_regions = ARRAY_SIZE(config.mem_regions),
            .num_irqchips = ARRAY_SIZE(config.irqchips),
            .num_pci_devices = ARRAY_SIZE(config.pci_devices),
    
            .vpci_irq_base = 140-32, 
        }},",
            fc.containerid
        );
    }
    return Ok(());
}

fn confighelperend(
    fc: &f2b::FrontendConfig,
    c: &mut Backendconfig,
    _ic: &f2b::ImageConfig,
) -> Result<(), Box<dyn Error>> {
    
    // To make possible also to use config-generator by its own, and not only by scripts,
    // Here a module which checks the regularity of "$crundir" should be implemented
    c.conf.push_str("\n};\n");

    // Modify the config file based on the operating system
    // TODO: Temporary mockup, to test linux conf we replace an already prepared .c for linux
    let pattern = r#"\.name = \".*\""#;
    let _re = Regex::new(pattern).unwrap();

    std::fs::write(&c.conffile, &c.conf)?;

    // jailhouse needs a .cell file
    // put the config.c by generate_guest_config in $JAILHOUSE/config dir and build
    let path_to_compile = format!("{}/tocompile.c", WORKPATH);
    std::fs::write(&path_to_compile, &c.conf)?;

    // Compile the config file
    //TODO: handle compilation error
    let _ = std::process::Command::new("make")
        .current_dir(format!("{}/", WORKPATH))
        .output()
        .expect("Failed to execute command");


    std::fs::copy(
        format!("{}/tocompile.cell", WORKPATH),
        &format!("{}/{}.cell", fc.crundir, fc.containerid),
    )?; // Copy the compiled cell file to the crundir
    return Ok(());
}


fn retrieve_state() -> Result<(Vec<String>, Vec<i8>, Vec<i8>, Vec<i8>), Box<dyn std::error::Error>> {
    let file_path = PathBuf::from(WORKPATH).join(STATEFILE);
    let content = fs::read_to_string(&file_path)?;
    let parsed_toml = content.parse::<Value>()?;
    
    let segments = parsed_toml
        .get("free_segments")
        .and_then(|section| section.get("segments"))
        .and_then(|seg| seg.as_array())
        .ok_or("Missing or invalid 'segments' field")?
        .iter()
        .filter_map(|s| s.as_str().map(String::from))
        .collect::<Vec<String>>();

    let bdf = parsed_toml
        .get("free_pci_devices_bdf")
        .and_then(|section| section.get("bdf"))
        .and_then(|b| b.as_array())
        .ok_or("Missing or invalid 'bdf' field")?
        .iter()
        .filter_map(|b| b.as_integer().and_then(|val| Some(val as i8)))
        .collect::<Vec<i8>>();

    let rcpus = parsed_toml
        .get("free_rcpus")
        .and_then(|section| section.get("ids"))
        .and_then(|ids| ids.as_array())
        .ok_or("Missing or invalid 'ids' field in 'free_rcpus'")?
        .iter()
        .filter_map(|id| id.as_integer().and_then(|val| Some(val as i8)))
        .collect::<Vec<i8>>();

    let fpga_regions = parsed_toml
        .get("free_fpga_regions")
        .and_then(|section| section.get("ids"))
        .and_then(|ids| ids.as_array())
        .ok_or("Missing or invalid 'ids' field in 'free_fpga_regions'")?
        .iter()
        .filter_map(|id| id.as_integer().and_then(|val| Some(val as i8)))
        .collect::<Vec<i8>>();

        Ok((segments, bdf, rcpus, fpga_regions))
}


fn save_state(
    fc_containerid: &str,
    c_segments: &Vec<String>,
    segments_before: &Vec<String>,
    c_rcpus: &Vec<i8>,
    c_fpga_regions: &Vec<i8>,
    bdf_used: Option<i8>,
    c_used_rcpus: &Vec<i8>,
    c_used_fpga_regions: &Vec<i8>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load the current state from state.toml
    let file_path = Path::new(WORKPATH).join(STATEFILE);
    let content = fs::read_to_string(&file_path)?;
    let mut parsed_toml: Value = content.parse::<Value>()?;

    // 1. Add `fc_containerid` to `ids` in `[containerid]`
    if let Some(containerid) = parsed_toml.get_mut("containerid") {
        if let Some(ids) = containerid.get_mut("ids").and_then(|ids| ids.as_array_mut()) {
            ids.push(Value::String(fc_containerid.to_string()));
        }
    }

    // 2. Replace `free_segments` with `c_segments`
    if let Some(free_segments) = parsed_toml.get_mut("free_segments") {
        free_segments["segments"] = Value::Array(c_segments.iter().map(|s| Value::String(s.clone())).collect());
    }

    // 3. Remove `bdf_used` from `free_pci_devices_bdf` if present and update `free_rcpus` with `c_rcpus` 
    if let Some(free_pci_devices_bdf) = parsed_toml.get_mut("free_pci_devices_bdf") {
        if let Some(bdf_array) = free_pci_devices_bdf.get_mut("bdf").and_then(|bdf| bdf.as_array_mut()) {
            if let Some(bdf_value) = bdf_used {
                bdf_array.retain(|b| b.as_integer() != Some(bdf_value as i64));
            }
        }
    }

    if let Some(free_rcpus) = parsed_toml.get_mut("free_rcpus") {
        free_rcpus["ids"] = Value::Array(c_rcpus.iter().map(|r| Value::Integer(*r as i64)).collect());
    }

    if let Some(free_fpga_regions) = parsed_toml.get_mut("free_fpga_regions") {
        free_fpga_regions["ids"] = Value::Array(c_fpga_regions.iter().map(|r| Value::Integer(*r as i64)).collect());
    }

    // 4. Add a new section for `fc_containerid`
    let new_container_section = {
        let mut container_data = Map::new();

        // Parse start and end addresses from `segments_before` and `c_segments`
        let (start_before, _end_before) = {
            let parts: Vec<&str> = segments_before[0].split(", ").collect();
            (
                u64::from_str_radix(parts[0].trim_start_matches("0x"), 16).expect("Invalid start address"),
                u64::from_str_radix(parts[1].trim_start_matches("0x"), 16).expect("Invalid end address"),
            )
        };

        let (start_after, _end_after) = {
            let parts: Vec<&str> = c_segments[0].split(", ").collect();
            (
                u64::from_str_radix(parts[0].trim_start_matches("0x"), 16).expect("Invalid start address"),
                u64::from_str_radix(parts[1].trim_start_matches("0x"), 16).expect("Invalid end address"),
            )
        };

        // Calculate the used memory range (segments_before - c_segments)
        let used_start = start_before;
        let used_end = start_after;
        //logging::log_message(logging::Level::Debug, &format!("Used memory start: 0x{:x}, end: 0x{:x}", used_start, used_end));

        // Format the result as a memory segment string
        let used_memory = format!("0x{:x}, 0x{:x}", used_start, used_end);
        //logging::log_message(logging::Level::Debug, &format!("Final used_memory string: {:?}", used_memory));

        // Insert into container_data
        container_data.insert("memory".to_string(), Value::String(used_memory));


        // Set `rcpus`
        let rcpus_value = if c_used_rcpus.is_empty() {
            "none".to_string()
        } else {
            c_used_rcpus.iter().map(|r| format!("{}", r)).collect::<Vec<_>>().join(", ")
        };
        container_data.insert("rcpus".to_string(), Value::String(rcpus_value));

        let fpga_regions_value = if c_used_fpga_regions.is_empty(){
            "none".to_string()
        }else{
            c_used_fpga_regions.iter().map(|r| format!("{}", r)).collect::<Vec<_>>().join(", ")
        };
        container_data.insert("fpga_regions".to_string(), Value::String(fpga_regions_value));

        // Set `pci_bdf`
        let pci_bdf_value = match bdf_used {
            Some(bdf) => format!("{}", bdf),
            None => "none".to_string(),
        };
        container_data.insert("pci_bdf".to_string(), Value::String(pci_bdf_value));

        // Attempt to create the Value::Table and add more debugging information
        match Value::try_from(container_data) {
            Ok(value) => {
                //logging::log_message(logging::Level::Debug, "Value::Table creation succeeded");
                value
            }
            Err(e) => {
                logging::log_message(logging::Level::Error, &format!("Value::Table creation failed: {}", e));
                return Err(Box::new(e));
            }
        }
    };

    // After successfully creating the Value::Table, assign it to parsed_toml
    //logging::log_message(logging::Level::Debug, &format!("Attempting to assign new_container_section to parsed_toml with key: {}", fc_containerid));

    // Ensure `parsed_toml` is a table and log its structure
    if let Some(parsed_table) = parsed_toml.as_table_mut() {
        //logging::log_message(logging::Level::Debug, "parsed_toml is a table, proceeding with assignment");

        // Assign the new container section
        parsed_table.insert(fc_containerid.to_string(), new_container_section);

        //logging::log_message(logging::Level::Debug, "Assignment to parsed_toml succeeded");
    } else {
        logging::log_message(logging::Level::Error, "parsed_toml is not a table and cannot be assigned to");
        return Err("parsed_toml is not a table".into());
    }

    // Log a success message after the assignment
    logging::log_message(logging::Level::Debug, "New container section added successfully in state.toml");

    // Save the updated TOML back to the file
    let updated_content = toml::to_string(&parsed_toml)?;
    fs::write(&file_path, updated_content)?;

    Ok(())
}