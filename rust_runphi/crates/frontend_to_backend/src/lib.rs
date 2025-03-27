//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (francesco.boccola@unina.it)
//*********************************************

use serde::Deserialize;
use serde_json;
use std::fs;

// This structure holds all the information mapped from the cli
// That basically means that are the flags from the OCI spec. We could pass directly
// the OCI structures, however a buffer structure like this allows for data modification
// before passing on to the backend (like cutting the ID down to 24 chars), and the
// backend only depends on this structure, that is much easier to control than the entire
// OCI structures (multiple variables do not actually make a lot of sense to consider
// in a partitioned container)
pub struct FrontendConfig {
    // Jsonconfig is the json parsed coming from the upper layer
    pub jsonconfig: serde_json::Value,
    pub crundir: String,
    pub containerid: String,
    pub bundle: String,
    pub mountpoint: String,
    pub guestconsole: String,
    pub pidfile: String,
}
impl FrontendConfig {
    pub fn new() -> Self {
        Self {
            jsonconfig: serde_json::Value::Null,
            crundir: String::new(),
            containerid: String::new(),
            bundle: String::new(),
            mountpoint: String::new(),
            guestconsole: String::new(),
            pidfile: String::new(),
        }
    }
}

// This structure holds the information that describe the image to be started as partitioned cell
// These are additional to standard information required by containers. For example, if dealing with a
// binary, the starting virtual address is required to perform a mapping, or the devices used or the
// binary to load in the cell
#[derive(Debug, Deserialize)]
pub struct ImageConfig {
    #[serde(default)]
    // Check if the container comes with its own Kernel and/or Ramdisk
    // runPHI requires that both the kernel and initrd are exposed by client through container's env variables KERNEL=/path/to/kernel_image, RAMDISK/path/to/initrd
    // moreover, in case of Jailhouse, the user should provide the path, in the container fs, of the inmate to run
    pub cpio: String,
    #[serde(default)]
    // if OSvar is "linux", the OS file contains the image of the linux kernel
    // if the OSvar contains anything else, like "zephyr", then a file integrating the runtime is loaded
    // When avialable, this variable could identify also a bare metal runtime like a WASM OS
    pub os_var: String,
    #[serde(default)]
    // When available, a custom kernel and ramkdisk shipped in the container can be specified. In this case the
    // application decides to bring its own kernel (platformm/board-dependent)
    pub kernel: String,
    #[serde(default)]
    pub ramdisk: String,
    // The inmate variable represents the file to be loaded containing the bare metal code or the
    // app with the libOS
    #[serde(default)]
    pub inmate: String,
    // The dtb is only for linux arm64, borderline case
    #[serde(default)]
    pub dtb: String,
    // Same here, the initrd is an alternative to ramdisk, depending on the arch
    #[serde(default)]
    pub initrd: String,
    #[serde(default)]
    pub netconf: String,
    // The starting_vaddress variable specifies the virtual address that the binary in inmate is
    // expecting to start. This determines how to remap the memory in the MMU when available, or
    // decides the placement when MMU not avaialble
    #[serde(default)]
    pub starting_vaddress: String,
    // This lines are needed to include the "net" and "rpu_req" field
    #[serde(default)]
    pub net: String,
    #[serde(default)]
    pub rpu_req: bool
    // TODO: handle default or missing values in a decent way
}
impl ImageConfig {
    pub fn get_from_file(mountpoint: &str) -> Self {
        // parsing configuration variables from the file
        //TODO: here is the case to parse also a node default used in the case the container does not specify this
        //TODO: parametrize boot boot.bin and config.json
        let json_str = match fs::read_to_string(format!("{}/boot/config.json", mountpoint)) {
            Ok(content) => content,
            Err(_) => String::new(),
        };
        let mut config: ImageConfig = serde_json::from_str(&json_str).unwrap();
        if !config.inmate.is_empty() {
            config.inmate = format!("{}{}", mountpoint, config.inmate).trim().to_string();
        } else {
            config.inmate = format!("{}/boot/boot.bin", mountpoint);
        }
        return config;
    }
}
