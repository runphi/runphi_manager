//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (francesco.boccola@unina.it)
//*********************************************

use regex::Regex;
use std::error::Error;
//use std::fs::{File, self, OpenOptions};
use std::fs;
use std::io::Write;
//use std::time::Instant;   //TIME CLOCK MONOTONIC
use std::process::Command;
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
use crate::configGenerator::templates::*;

#[allow(unused_imports)]
use logging::timer;

use f2b::paths::WORKPATH;

const STATEFILE: &str = "state.toml";
const CONFIG_FILE: &str = "platform_info.toml";


// This structure holds all the information related to the configuration of the partitioned container
// There is the configuration file, the configuration string, and needed variables for resources,
// like cpus, memory addresses, devices, and in general all the output of the configGeneration phase
#[derive(Debug)]
pub struct BackendConfig {
    pub conf: String,
    pub cpus: u8,
    pub conffile: PathBuf,
    pub net: String,
    pub rpu_req: bool,
    pub segments: Vec<String>,
    pub bdf: Vec<i8>,
    pub rcpus: Vec<i8>,
    pub used_rcpus: Vec<i8>,
    // platform_info.toml describes the R5 cores ([rcpu.<id>])
    pub rcpu_layout: bool,
    // the R5 core of an R5 cell, on such a platform
    pub rcpu: Option<rpu::RcpuInfo>,
    // the memory the cell takes from the free segments (start, end)
    pub memory: Option<(u64, u64)>,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl BackendConfig {
    // Constructor function
    pub fn new() -> Self {
        Self {
            conf: String::new(),
            cpus: 0,
            conffile: PathBuf::new(),
            net: String::new(),
            rpu_req: false,
            segments: Vec::new(),
            bdf: Vec::new(),
            rcpus: Vec::new(),
            used_rcpus: Vec::new(),
            rcpu_layout: false,
            rcpu: None,
            memory: None,
        }
    }
}

// Every step's error is returned: a configuration that is not complete would
// not compile (or would describe another cell), and the container must not be
// created. The state (state.toml) is only written once the cell file is built.
pub fn config_generate(fc: &f2b::FrontendConfig) -> Result<Box<f2b::ImageConfig>, Box<dyn Error>> {
    logging::log_message(logging::Level::Debug, format!("Starting config generator for id {}", &fc.containerid).as_str());

    //let start = timer::capture(); //TIMELINE

    let mut c = BackendConfig::new();
    c.conffile = fc.crundir.join(format!("config{}.conf", fc.containerid));

    // parsing configuration variables from the file
    //THIS IS THE ACCESS TO JSON.CONFIG FROM DOCKER
    logging::log_message(logging::Level::Debug, format!("Reading the config.json inside the container for id {}", &fc.containerid).as_str());
    let mut config = Box::new(f2b::ImageConfig::get_from_file(&fc.mountpoint)?);
    logging::log_message(logging::Level::Debug, format!("The mountpoint for the container with id {} is {}", &fc.containerid, fc.mountpoint.display()).as_str());
    // A Linux guest needs a cell configuration for "jailhouse cell linux",
    // which this backend does not generate yet
    if config.os_var == "linux" {
        return Err("os_var \"linux\": Linux cells are not implemented in backend_jailhouse".into());
    }

    //Clone the value of config.net (from the internal .json) to c.net
    c.net = config.net.clone();

    // Do we require rpus?
    c.rpu_req = config.rpu_req;

    // Read the state of the machine from the state.toml file (in particular free memory and free bdfs)
    //let start = timer::capture(); //TIMELINE
    let (segments, bdf, rcpus) = retrieve_state()?;
    //let end = timer::capture(); //TIMELINE
    //timer::log_elapsed(start, end, "Retrieve state")?;
    // Update the struct
    c.segments = segments;
    c.bdf = bdf;
    c.rcpus = rcpus;
    //c.preamble = preamble;

    // The R5 cores of the platform, if platform_info.toml describes them
    let platform: Value = fs::read_to_string(Path::new(WORKPATH).join(CONFIG_FILE))?.parse()?;
    let platform_rcpus = rpu::platform_rcpus(&platform)?;
    c.rcpu_layout = !platform_rcpus.is_empty();

    logging::log_message(logging::Level::Debug, format!("Config helper start for id {}", &fc.containerid).as_str());
    //let start = timer::capture(); //TIMELINE
    confighelperstart(fc, &mut c, &config)?;
    //let end = timer::capture(); //TIMELINE
    //timer::log_elapsed(start, end, "helperstart")?;

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

    // cpus is a floating point number
    // If the backend does not support fractional allots, that's a backend matter
    let mut cpus: f64 = quota / period;

    /*
     Here can be implemented: hypervisor agnostic real-time schedulability tests, etc.
    */

    logging::log_message(logging::Level::Debug, format!("Configuring CPU for id {}", &fc.containerid).as_str());
    //let start = timer::capture(); //TIMELINE
    //If rpu_req is true we are requesting RPUs and not CPUs
    //c.rpu_req=true; //FOR TESTING PURPOSES ALWAYS ALLOCATE RPUs, COMMENT IN THE FINAL CODE
    if c.rpu_req {
        // One R5 core: the one whose memory holds the program (remoteproc loads
        // it at its link address), or the first free core if the program only
        // uses the TCMs or the platform does not describe the cores
        cpus = 0.0;
        cpu::cpuconf(fc, &mut c, &quota, &period, &cpus)?;
        let elf_core = if c.rcpu_layout {
            rpu::rcpu_for_elf(Path::new(&config.inmate), &platform_rcpus)?
        } else {
            None
        };
        let rcpu = match elf_core {
            Some(id) => id,
            None => *c.rcpus.first().ok_or("no free R5 core")?,
        };
        rpu::rpuconf(&mut c, rcpu)?;
        c.rcpu = platform_rcpus.iter().find(|r| r.id == rcpu).cloned();
        if c.rcpu_layout && c.rcpu.is_none() {
            return Err(format!("R5 core {} is not described in platform_info.toml", rcpu).into());
        }
    } else {
        cpu::cpuconf(fc, &mut c, &quota, &period, &cpus)?;
    }
    //logging::log_message(logging::Level::Debug, format!("\nconfiguration after cpuconf is  {}", c.conf).as_str());
    //let end = timer::capture(); //TIMELINE
    //timer::log_elapsed(start, end, "CPU Configuration")?;
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

    //Save the value of the bdf we use
    let bdf_used = if c.net != "none" {
        c.bdf.iter().min().cloned()
    } else {
        None
    };

    //Pass everything to memconfig
    logging::log_message(logging::Level::Debug, format!("Configuring memory for id {}", &fc.containerid).as_str());
    
    //let start = timer::capture(); //TIMELINE
    mem::memconfig(&mut c, &mem_request_hex)?;
    //let end = timer::capture(); //TIMELINE
    //timer::log_elapsed(start, end, "Memory Configuration")?;

    logging::log_message(logging::Level::Debug, format!("Configuring Device for id {}", &fc.containerid).as_str());
    
    //let start = timer::capture(); //TIMELINE
    device::devconfig(&mut c)?;
    //let end = timer::capture(); //TIMELINE
    //timer::log_elapsed(start, end, "Device Configuration")?;

    boot::bootconfbackend(fc, &mut config)?;

    //TODO: call net config here (take net memory areas from memory)

    // Guest console is allocated when -t flag is provided
    // useful for Hypervisor like XEN or BAO which give the possibility
    // to start Guest with fully fledged OS
    // Jailhouse gives only the option to start non-root linux cell but the user can connect to it
    // only through ssh

    // If -t flag was specified, call COMMUNICATION backend for further processing
    // E.G. allocate terminal or ssh shell

    if !fc.guestconsole.as_os_str().is_empty() {
        let mut file = fs::File::create(fc.crundir.join("console"))?;
        writeln!(file, "{}", fc.guestconsole.display())?;
    }

    //let _ = communication::communicationconfig(&mut c); //communication è stato incluso direttamente nel preamble

    logging::log_message(logging::Level::Trace, format!("\nactual configuration is  {}", c.conf).as_str());

    // Build the cell file, then take the resources in state.toml
    confighelperend(fc, &mut c, &config)?;
    save_state(
        &fc.containerid,
        &c.segments,
        c.memory,
        &c.rcpus,
        bdf_used,
        &c.used_rcpus,
    )?;
    logging::log_message(logging::Level::Debug, format!("State saved successfully for id {}", &fc.containerid).as_str());

    Ok(config)
}

fn confighelperstart(
    fc: &f2b::FrontendConfig,
    c: &mut BackendConfig,
    ic: &f2b::ImageConfig,
) -> Result<(), Box<dyn Error>> {
    // Write the conf file preamble. Every guest but Linux (Zephyr, a bare-metal
    // binary, ...) is a single image that `jailhouse cell load` puts in the cell,
    // as in createguest(): they all get the same preamble.

    if ic.os_var != "linux" {

        logging::log_message(logging::Level::Debug, format!("Starting helper start for id {}", &fc.containerid).as_str());
        // Construct the full path to the TOML file
        let config_path = Path::new(WORKPATH).join(CONFIG_FILE);

        // Read the contents of the TOML file
        let config_content = fs::read_to_string(config_path)?;

        // Parse the content as TOML
        let parsed_toml: Value = config_content.parse::<Value>()?;

        logging::log_message(logging::Level::Debug, format!("Retrieving preamble for id {}", &fc.containerid).as_str());
        // Retrieve the `preamble` value under `[jailhouse_preamble]`
        if let Some(preamble) = parsed_toml
            .get("jailhouse_preamble")
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
            return Err("Field 'preamble' not found in [jailhouse_preamble]".into());
        }

    } else if ic.os_var == "linux" { //TODO: test and implement linux OS cell
    //     c.conf = format!(
    //         "#include \"cell.h\"
    // struct {{
    //     struct jailhouse_cell_desc cell;
    // }} __attribute__((packed)) config = {{
    //     .cell = {{
    //         .signature = JAILHOUSE_CELL_DESC_SIGNATURE,
    //         .revision = JAILHOUSE_CONFIG_REVISION,
    //         .name = \"{}\",
    //         .flags = JAILHOUSE_CELL_PASSIVE_COMMREG |
    //             JAILHOUSE_CELL_VIRTUAL_CONSOLE_PERMITTED,
    //         .cpu_set_size = sizeof(config.cpus),
    //         .num_memory_regions = ARRAY_SIZE(config.mem_regions),
    //         .num_irqchips = ARRAY_SIZE(config.irqchips),
    //         .num_pci_devices = ARRAY_SIZE(config.pci_devices),
    
    //         .vpci_irq_base = 140-32, 
    //     }},",
    //         fc.containerid
    //     );
    }
    Ok(())
}

fn confighelperend(
    fc: &f2b::FrontendConfig,
    c: &mut BackendConfig,
    _ic: &f2b::ImageConfig,
) -> Result<(), Box<dyn Error>> {
    
    // Append closing brace to the configuration
    c.conf.push_str("\n};\n");

    // Modify the config file based on the operating system (mockup example)
    let pattern = r#"\.name = \".*\""#;
    let _re = Regex::new(pattern).unwrap();

    // Write the configuration to the .c file
    //std::fs::write(&c.conffile, &c.conf)?;

    // Define paths in fc.crundir, with the correct naming convention
    let path_to_compile = fc.crundir.join("tocompile.c");
    let cell_file_path = fc.crundir.join(format!("{}.cell", fc.containerid));
    let obj_file_path = fc.crundir.join(format!("{}.o", fc.containerid));

    // Write the config to tocompile.c in fc.crundir
    std::fs::write(&path_to_compile, &c.conf)?;

    // Compile the .c file to .o
    let compile_status = Command::new("gcc")
        .args([
            // Jailhouse's headers are GNU C11: in C23, gcc 15's default, `bool`
            // and `true` are keywords and types.h's `enum { true = 1, ... } bool`
            // does not compile.
            "-std=gnu11",
            "-Werror",
            "-Wall",
            "-Wextra",
            // The signature (JAILHOUSE_CELL_DESC_SIGNATURE, "JHCELL") fills its
            // 6-byte array without the NUL on purpose; gcc 15 warns about it.
            // Older gcc ignore the unknown -Wno- option.
            "-Wno-unterminated-string-initialization",
            "-D__LINUX_COMPILER_TYPES_H",
            "-I/usr/share/runPHI/include",   // Specify the absolute include path
            "-c",                            // Compile without linking
            path_to_compile.to_str().unwrap(),
            "-o",
            obj_file_path.to_str().unwrap(),
        ])
        .output()?;

    if !compile_status.status.success() {
        // gcc's first errors, the rest is in the log
        let stderr = String::from_utf8_lossy(&compile_status.stderr);
        logging::log_message(
            logging::Level::Error,
            &format!("Compilation of the cell configuration failed:\n{}\n{}", stderr, c.conf),
        );
        let first: Vec<&str> = stderr.lines().filter(|l| l.contains("error")).take(3).collect();
        return Err(format!(
            "the cell configuration of {} does not compile: {}",
            fc.containerid,
            first.join("; ")
        )
        .into());
    }

    // Convert the .o file to .cell directly in fc.crundir
    let objcopy_status = Command::new("objcopy")
        .args([
            "-O",
            "binary",
            "--remove-section=.note.gnu.property",
            obj_file_path.to_str().unwrap(),
            cell_file_path.to_str().unwrap(),
        ])
        .status()?;

    if !objcopy_status.success() {
        logging::log_message(logging::Level::Error, "Conversion to .cell file failed!!!");
        return Err("Conversion to .cell file failed".into());
    }

    Ok(())
}

// (free memory segments, free PCI BDFs, free RPU IDs)
type StateSnapshot = (Vec<String>, Vec<i8>, Vec<i8>);

fn retrieve_state() -> Result<StateSnapshot, Box<dyn std::error::Error>> {
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
        .filter_map(|b| b.as_integer().map(|val| val as i8))
        .collect::<Vec<i8>>();

    let rcpus = parsed_toml
        .get("free_rcpus")
        .and_then(|section| section.get("ids"))
        .and_then(|ids| ids.as_array())
        .ok_or("Missing or invalid 'ids' field in 'free_rcpus'")?
        .iter()
        .filter_map(|id| id.as_integer().map(|val| val as i8))
        .collect::<Vec<i8>>();

        Ok((segments, bdf, rcpus))
}


fn save_state(
    fc_containerid: &str,
    c_segments: &[String],
    memory: Option<(u64, u64)>,
    c_rcpus: &[i8],
    bdf_used: Option<i8>,
    c_used_rcpus: &[i8],
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

    // 4. Add a new section for `fc_containerid`
    let new_container_section = {
        let mut container_data = Map::new();

        // The memory taken from the free segments ("none": an R5 cell, whose
        // memory is its core's region)
        let used_memory = match memory {
            Some((start, end)) => format!("0x{:x}, 0x{:x}", start, end),
            None => "none".to_string(),
        };

        // Insert into container_data
        container_data.insert("memory".to_string(), Value::String(used_memory));


        // Set `rcpus`
        let rcpus_value = if c_used_rcpus.is_empty() {
            "none".to_string()
        } else {
            c_used_rcpus.iter().map(|r| format!("{}", r)).collect::<Vec<_>>().join(", ")
        };
        container_data.insert("rcpus".to_string(), Value::String(rcpus_value));

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

// TEST MODULE 
/* #[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let fc = ..; // Initialize fc
        let image_config = ..; // Expected result
        let result = config_generate(&fc);

        match result {
            Ok(config) => assert_eq!(config, image_config),
            Err(e) => panic!("Test failed with error: {:?}", e),
        }
    }
} */
