//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (f.boccola@studenti.unina.it)
//*********************************************

use regex::Regex;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
//use std::time::Instant;   //TIME CLOCK MONOTONIC

use f2b;
pub mod boot;
pub mod communication;
pub mod cpu;
pub mod fpga;
pub mod device;
pub mod mem;
pub mod network;

const WORKPATH: &str = "/usr/share/runPHI";
//const RUNDIR: &str = "/run/runPHI";

// This structure holds all the information related to the configuration of the partitioned container
// There is the configuration file, the configuration string, and needed variables for resources,
// like cpus, memory addresses, devices, and in general all the output of the configGeneration phase
#[derive(Debug)]
pub struct Backendconfig {
    pub conf: String,
    pub cpus: u8,
    pub fpga_regions: u8,
    pub conffile: String,
    pub net: String,
}

impl Backendconfig {
    // Constructor function
    pub fn new() -> Self {
        Self {
            conf: String::new(),
            cpus: 0,
            fpga_regions: 0,
            conffile: String::new(),
            net: String::new(),
        }
    }
}

//TODO: error handling across this function is a box of shit, handle it
pub fn config_generate(fc: &f2b::FrontendConfig) -> Result<Box<f2b::ImageConfig>, Box<dyn Error>> {

    let _ = append_message_with_time(&format!("starting config generator")); //TIME
    let mut c = Backendconfig::new();
    c.conffile = format!("{}/config{}.conf", fc.crundir, fc.containerid);

    // parsing configuration variables from the file
    //THIS IS THE ACCESS TO JSON.CONFIG FROM DOCKER
    //writeln!(logfile, "Parsing config.json")?;                        //DEBUG
    let mut config = Box::new(f2b::ImageConfig::get_from_file(&fc.mountpoint));
    //Clone the value of config.net (from the internal .json) to c.net
    c.net = config.net.clone();

    //let start_time = Instant::now();                                                    //TIME
    let _ = append_message_with_time(&format!("helper start")); //TIME
    let _ = confighelperstart(fc, &mut c, &config);
    let _ = append_message_with_time(&format!("helper start end")); //TIME
    //let _ = append_message_with_time(&format!("Time elapsed in helper start is: {:?}", start_time.elapsed())); //TIME


    // This region of code could be extended with code to retrieve other specific Docker's flags which set CPU limitations
    // cpus where allow guest execution set by Docker's flag 'cpuset-cpus'
    // If flag is not set, let's go for 1. It will be overwritten by quota and period if they are defined
    let _cpu_set = fc.jsonconfig["linux"]["resources"]["cpu"]["cpus"]
        .as_f64()
        .unwrap_or(1.0);

    //writeln!(logfile, "Got cpu_set")?;                    //DEBUG

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

    //writeln!(logfile, "Got period quota {} {}", period, quota)?;       //DEBUG

    // cpus is a floating point number
    // If the backend does not support fractional allots, that's a backend matter
    let cpus: f64 = quota / period;

    /*
     Here can be implemented: hypervisor agnostic real-time schedulability tests, etc.
    */

    //writeln!(logfile, "Calling CPU config")?;                         //DEBUG
    //let start_time = Instant::now();                                                    //TIME
    let _ = cpu::cpuconf(fc, &mut c, &quota, &period, &cpus);
    let _ = append_message_with_time(&format!("Finished cpu config")); //TIME
    //This region of code could be extended through code to retrieve other specific Docker's flags which set MEM limitations
    // Extract values from the JSON structure
    //In the json structure only limit is created by kubernetes memory reservation doesn't exist so I'll comment it
    /* let _mem_res = fc.jsonconfig["linux"]["resources"]["memory"]["reservation"] //Domain memory in MB, (--memory-reservation="")
    .as_u64() // Assuming memory values are in unsigned integers
    .unwrap_or(512); // Set default value to 512 MB if the value is missing */

    //before requesting memory, let's get info about the bitstreams
    let _ = fpga::fpgaconf(fc, &mut c, &mut config.accelerator);
    let _ = append_message_with_time(&format!("Finished fpga config")); //TIME
    let _ = append_message_with_time(&format!("accelerator.bitstream: {}",config.accelerator.bitstream)); //TIME


    let mem_request = fc.jsonconfig["linux"]["resources"]["memory"]["limit"] //Maximum domain memory in MB, (-m, --memory="")
        .as_u64() // Assuming memory values are in unsigned integers
        .unwrap_or(67_108_864); // Set default value to 64 MiB if the value is missing
                                //writeln!(logfile, "Mem_Requested is: {}", mem_request)?;       //DEBUG

    //Convert the value parsed to a hexadecimal String
    let mem_request_hex = format!("0x{:x}", mem_request);

    //let start_time = Instant::now();                                                    //TIME

    //Pass everything to memconfig
    let _ = append_message_with_time(&format!("starting mem config")); //TIME
    let _ = append_message_with_time(&mem_request_hex); //TIME
    let _ = mem::memconfig(&mut c, &mem_request_hex);
    let _ = append_message_with_time(&format!("finished mem config")); //TIME

    //let _ = append_message_with_time(&format!("Time elapsed in mem config is: {:?}", start_time.elapsed())); //TIME

    //writeln!(logfile, "Memoryend - devconfig start")?;       //DEBUG

    //let start_time = Instant::now();                                                    //TIME

    let _ = append_message_with_time(&format!("starting dev cfg")); //TIME
    let _ = device::devconfig(&mut c);
    let _ = append_message_with_time(&format!("finished dev cfg")); //TIME
    //let _ = append_message_with_time(&format!("Time elapsed in dev config is: {:?}", start_time.elapsed())); //TIME

    //let start_time = Instant::now();                                                    //TIME

    let _ = boot::bootconfbackend(fc, &mut config);

    //let _ = append_message_with_time(&format!("Time elapsed in boot config is: {:?}", start_time.elapsed())); //TIME


    //TODO: call net config here (take net memory areas from memory)

    // Guest console is allocated when -t flag is provided
    // useful for Hypervisor like XEN or BAO which give the possibility
    // to start Guest with fully fledged OS
    // Jailhouse gives only the option to start non-root linux cell but the user can connect to it
    // only through ssh

    // If -t flag was specified, call COMMUNICATION backend for further processing
    // E.G. allocate terminal or ssh shell
    //writeln!(logfile, "Before guestconsole is empty")?; //DEBUG
    if !fc.guestconsole.is_empty() {
        let mut file = fs::File::create(format!("{}/console", fc.crundir))
            .expect("Failed to create console file");
        writeln!(file, "{}", fc.guestconsole).expect("Failed to write console file");
    }

    //let start_time = Instant::now();                                                    //TIME

    let _ = communication::communicationconfig(&mut c);

    //let _ = append_message_with_time(&format!("Time elapsed in comm config is: {:?}", start_time.elapsed())); //TIME

    //writeln!(logfile, "calling config helper end")?; //DEBUG

    //let start_time = Instant::now();                                                    //TIME

    let _ = confighelperend(fc, &mut c, &config);
    //let _ = append_message_with_time(&format!("Time elapsed in helper_end is: {:?}", start_time.elapsed())); //TIME

    //writeln!(logfile, "last line")?; //DEBUG

    // Write the return variable 'config' to the logfile
    //writeln!(logfile, "State of 'c': {:?}", c)?; //DEBUG

    return Ok(config);
}

fn confighelperstart(
    fc: &f2b::FrontendConfig,
    c: &mut Backendconfig,
    ic: &f2b::ImageConfig,
) -> Result<(), Box<dyn Error>> {
    // Write the generic header to the conf file
    //let mut file = File::create(conf)?;
    //TODO: IRQ base must be replaced
    //writeln!(
    //    file,

    if ic.os_var == "zephyr" {
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
        .cpu_reset_address = 0x70000000, 
    }},",
            fc.containerid
        );
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
    /* let mut logfile = fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open("/usr/share/runPHI/log_helperend.txt")?; */

    //writeln!(logfile, "first line of helperend")?; //DEBUG

    // To make possible also to use config-generator by its own, and not only by scripts,
    // Here a module which checks the regularity of "$crundir" should be implemented
    c.conf.push_str("\n};\n");

    // Modify the config file based on the operating system
    // TODO: Temporary mockup, to test linux conf we replace an already prepared .c for linux
    let pattern = r#"\.name = \".*\""#;
    let _re = Regex::new(pattern).unwrap();

    //writeln!(logfile, "after regex of helperend")?; //DEBUG

    std::fs::write(&c.conffile, &c.conf)?;
    //writeln!(logfile, "before tocompile.c = zephyr of helperend")?; //DEBUG

    // jailhouse needs a .cell file
    // put the config.c by generate_guest_config in $JAILHOUSE/config dir and build
    let path_to_compile = format!("{}/tocompile.c", WORKPATH);
    std::fs::write(&path_to_compile, &c.conf)?;
    //writeln!(logfile, "after tocompile.c = zephyr of helperend")?; //DEBUG

    // Compile the config file
    //TODO: handle compilation error
    let output = std::process::Command::new("make")
        .current_dir(format!("{}/", WORKPATH))
        .output()?;
    if !output.status.success() {
        println!("Command failed: {}", String::from_utf8_lossy(&output.stderr));
    } 
    std::fs::copy(
        format!("{}/tocompile.cell", WORKPATH),
        &format!("{}/{}.cell", fc.crundir, fc.containerid),
    )?; // Copy the compiled cell file to the crundir
        //writeln!(logfile, "last line of helperend")?; //DEBUG
    return Ok(());
}

#[allow(dead_code)]
fn append_message_with_time(message: &str) -> Result<(), Box<dyn Error>> {  //TIME

    // Open the file in append mode, create it if it doesn't exist
    let mut timefile = OpenOptions::new()
    .create(true)
    .append(true)
    .open("/usr/share/runPHI/times_file.txt")?;
    
    // Write the message and current time to the file, separated by an equal sign
    writeln!(timefile, "{}", message)?;
    
    Ok(())
}
