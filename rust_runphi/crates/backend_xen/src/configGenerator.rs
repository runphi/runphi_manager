//*********************************************
// Authors: Francesco Boccola (francesco.boccola@unina.it)
//*********************************************

use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use f2b;
pub mod boot;
pub mod communication;
pub mod cpu;
pub mod device;
pub mod mem;
pub mod network;

//const WORKPATH: &str = "/usr/share/runPHI";
//const RUNDIR: &str = "/run/runPHI";
//(fc.crundir = run/runPHI/containerid)
//in questo caso stiamo utilizzando LVM per gestire i dischi guest
const LVM_GROUP_NAME: &str = "test-vg";

// This structure holds all the information related to the configuration of the partitioned container
// There is the configuration file, the configuration string, and needed variables for resources,
// like cpus, memory addresses, devices, and in general all the output of the configGeneration phase
#[derive(Debug)]
pub struct BackendConfig {
    pub conf: String,
    pub cpus: u8,
    pub conffile: PathBuf,
    pub net: String,
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
        }
    }
}

//TODO: error handling across this function is a box of shit, handle it
pub fn config_generate(fc: &f2b::FrontendConfig) -> Result<Box<f2b::ImageConfig>, Box<dyn Error>> {

    //log_timestamp("Config_Gen_Start")?; //Just for boot times timeline extraction
    // Equivalent using the logging::timer module (reads /dev/arm_timer once, logs to LOG_PATH):
    //let _cfg_gen_start = logging::timer::capture();
    //let _ = logging::timer::log_phase("Config_Gen_Start");

    logging::log_message(logging::Level::Info,  "starting config generator".to_string().as_str());
    let mut c = BackendConfig::new();
    c.conffile = fc.crundir.join("config.cfg");
    logging::log_message(logging::Level::Debug,  format!("Target file path : {}", c.conffile.display()).as_str());

    // parsing configuration variables from the file
    //THIS IS THE ACCESS TO JSON.CONFIG FROM DOCKER
    //writeln!(logfile, "Parsing config.json")?;                        //DEBUG
    let mut config = Box::new(f2b::ImageConfig::get_from_file(&fc.mountpoint)?);

    

    let _ = confighelperstart(fc, &mut c, &config);
    logging::log_message(logging::Level::Debug, "Finished helper start".to_string().as_str());
    boot::bootconf(fc, &mut c, &mut config);


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

    let _ = cpu::cpuconf(fc, &mut c, &quota, &period, &cpus);
    logging::log_message(logging::Level::Debug,  "Finished cpu config".to_string().as_str());
    //This region of code could be extended through code to retrieve other specific Docker's flags which set MEM limitations

    // Extract values from the JSON structure
    //In the json structure only limit is created by kubernetes memory reservation doesn't exist, (but it cluod be specified in other way??), anyway we need to cuild the lv for the vm
    let st_req = fc.jsonconfig["linux"]["resources"]["memory"]["reservation"] //Domain memory in MB, (--memory-reservation="")
    .as_u64() // Assuming memory values are in unsigned integers
    .unwrap_or(32); // Set default value to 512 MB if the value is missing

    let mem_request = fc.jsonconfig["linux"]["resources"]["memory"]["limit"] //Maximum domain memory in MB, (-m, --memory="")
        .as_u64() // Assuming memory values are in unsigned integers
        .unwrap_or(32); // Set default value to 512M if the value is missing
    
    //Pass everything to memconfig    
    logging::log_message(logging::Level::Debug,  format!("Memory request: {} MB, Memory reservation: {} MB", mem_request, st_req).as_str());
    let _ = mem::memconf(&mut c,&st_req, &mem_request,LVM_GROUP_NAME);
    logging::log_message(logging::Level::Debug,  "Finished mem config".to_string().as_str());

    //-------------------------------------------------------------------------------------
    //In xen physical device are managed by dom0 - unless u wanto to set PCI passthroug
    //-------------------------------------------------------------------------------------
    //let _ = device::devconfig(&mut c);

    let _ = network::netconfig(&mut c);
    logging::log_message(logging::Level::Debug,  "Finished network config".to_string().as_str());
    //------------------------------------------------------------------------------------
    //If u want to write the console u have to specify this file in the create command in lib 
    //by sending this command "xl console container_id >> "$output_file" 2>&1 &"
    //the console is entirly wrote in the output file  
    //------------------------------------------------------------------------------------

    if !fc.guestconsole.as_os_str().is_empty() {
        let mut file = fs::File::create(fc.crundir.join("console"))?;
        writeln!(file, "{}", fc.guestconsole.display())?;
    }


    //------------------------------------------------------------------------------------
    //The comuniccation between the doms, it should be possible simply by the virtaual network interface
    //------------------------------------------------------------------------------------
    //let _ = communication::communicationconfig(&mut c);
    let _ = confighelperend(fc, &mut c, &config);
    logging::log_message(logging::Level::Info,  "Finished config generation".to_string().as_str());
    logging::log_message(logging::Level::Debug,  format!("Config file generated at: {}", c.conffile.display()).as_str());
    logging::log_message(logging::Level::Debug,  format!("Config generation is:\n{}", c.conf).as_str());
    //log_timestamp("Config_Gen_End")?; //Just for boot times timeline extraction
    // Equivalent using the logging::timer module:
    //let _ = logging::timer::log_phase("Config_Gen_End");
    // Or, to also record the elapsed duration since _cfg_gen_start:
    //let _cfg_gen_end = logging::timer::capture();
    //let _ = logging::timer::log_elapsed(_cfg_gen_start, _cfg_gen_end, "config_generate");

    Ok(config)
}

fn confighelperstart(
    fc: &f2b::FrontendConfig,
    c: &mut BackendConfig,
    _ic: &f2b::ImageConfig,
) -> Result<(), Box<dyn Error>> {

    // Write the generic header to the conf file
    c.conf = format!("
#---------------------------------------------------------------
#Configuration file for container with id : {} 
#---------------------------------------------------------------
name = \"{}\" \n"
,fc.containerid,fc.containerid);

    Ok(())
}

fn confighelperend(
    _fc: &f2b::FrontendConfig,
    c: &mut BackendConfig,
    _ic: &f2b::ImageConfig,
) -> Result<(), Box<dyn Error>> {

    // Add the end of the configuration file
    c.conf.push_str("gic_version=\"v2\"\non_crash=\"preserve\"\n");

    //create and write the file
    std::fs::write(&c.conffile, &c.conf)?;

    Ok(())
}

#[allow(dead_code)]
fn log_timestamp(message: &str) -> std::io::Result<()> {
    let timestamp = fs::read_to_string("/dev/arm_timer")?;
    let timestamp = timestamp.trim();
    
    // Append to your timestamp file
    let log_entry = format!("{} - {}\n", timestamp, message);
    
    // Use OpenOptions to append instead of overwriting
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/root/boot_times_raw_data.txt")?;
    
    file.write_all(log_entry.as_bytes())?;
    
    Ok(())
}