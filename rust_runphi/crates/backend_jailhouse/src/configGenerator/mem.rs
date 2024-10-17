//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (f.boccola@studenti.unina.it)
//*********************************************

use regex::Regex;
use std::error::Error;
use std::fs::{OpenOptions}; //DEBUG
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

use crate::configGenerator;
//use f2b;

const WORKPATH: &str = "/usr/share/runPHI";
const PCI_IVSHMEM_ID_FILE: &str = "pci_ivshmem_id.txt";

// Function to check if a file is empty
fn is_file_empty(file_path: &str) -> io::Result<bool> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);
    let mut first_line = String::new();
    Ok(reader.read_line(&mut first_line)? == 0)
}

// Function to check if a file is empty
fn is_path_empty(file_path: &Path) -> io::Result<bool> {
    let file = File::open(file_path)?;
    let metadata = file.metadata()?;
    Ok(metadata.len() == 0)
}

fn count_mem_regions(config: &str) -> usize {
    // Count the occurrences of the memory region blocks
    let region_count = config
        .matches("/* IVSHMEM shared memory region (demo)")
        .count()
        + config.matches("/* UART */").count()
        + config.matches("/* RAM */").count()
        + config.matches("/* communication region */").count();

    // Count the occurrences of the macro and add the expanded count
    let macro_count = config.matches("JAILHOUSE_SHMEM_NET_REGIONS").count() * 4;

    // Sum the counts
    region_count + macro_count
}

// Function to initialize the free_segments.txt file
fn initialize_free_segments(file_path: &str, free_segments: &[(i64, i64)]) -> io::Result<()> {
    /* let mut logfile = OpenOptions::new()
    .create(true)
    .append(true)
    .open("/usr/share/runPHI/log_mem.txt")?; */

    //writeln!(logfile, "initialize_free_segments start")?; //DEBUG

    let mut writer = File::create(file_path)?;
    for &(start, end) in free_segments {
        writeln!(writer, "0x{:x}, 0x{:x}", start, end)?;
    }

    //writeln!(logfile, "initialize_free_segments end")?; //DEBUG

    Ok(())
}

// Function to read segments from free_segments.txt
fn read_free_segments(file_path: &str, free_segments: &mut Vec<(i64, i64)>) -> io::Result<()> {
    /* let mut logfile = OpenOptions::new()
    .create(true)
    .append(true)
    .open("/usr/share/runPHI/log_mem.txt")?; */

    //writeln!(logfile, "read_free_segments start")?; //DEBUG
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() == 2 {
            if let (Ok(start), Ok(end)) = (
                i64::from_str_radix(parts[0].trim_start_matches("0x"), 16),
                i64::from_str_radix(parts[1].trim_start_matches("0x"), 16),
            ) {
                free_segments.push((start, end));
            }
        }
    }
    //writeln!(logfile, "read_free_segments end")?; //DEBUG
    Ok(())
}

// Function to obtain memory space from available_memory.txt
fn obtain_memory_space(file_path: &str, free_segments: &mut Vec<(i64, i64)>) -> io::Result<()> {
    /* let mut logfile = OpenOptions::new()
    .create(true)
    .append(true)
    .open("/usr/share/runPHI/log_mem.txt")?; */

    //writeln!(logfile, "obtain_memory_space start")?; //DEBUG

    let file = File::open(file_path)?;

    //writeln!(logfile, "obtain_memory_space opened file")?; //DEBUG

    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() == 2 {
            if let (Ok(start), Ok(end)) = (
                i64::from_str_radix(parts[0].trim_start_matches("0x"), 16),
                i64::from_str_radix(parts[1].trim_start_matches("0x"), 16),
            ) {
                free_segments.push((start, end));
            }
        }
    }
    //writeln!(logfile, "obtain_memory_space end")?; //DEBUG
    Ok(())
}

// Function to populate memory_regions_sizes from the config string
fn populate_memory_regions_sizes(config: &str, memory_regions_sizes: &mut Vec<i64>) {
    let lines = config.lines();
    let mut add_size = false;

    for line in lines {
        let trimmed_line = line.trim();

        if trimmed_line.starts_with("/* UART */")
            || trimmed_line.starts_with("/* communication region */")
            || trimmed_line.starts_with("/* IVSHMEM shared memory region (demo) */")
        {
            add_size = false;
        } else if trimmed_line.contains("/*") {
            add_size = true;
        }

        if add_size && trimmed_line.starts_with(".size") {
            if let Some(size_str) = trimmed_line.split('=').nth(1) {
                if let Ok(size) = i64::from_str_radix(
                    size_str
                        .trim()
                        .trim_end_matches(',')
                        .trim_start_matches("0x"),
                    16,
                ) {
                    memory_regions_sizes.push(size);
                }
            }
            add_size = false;
        }
    }
}

// Function to check if at least one of the memory areas in free_segments is big enough
fn check_memory_areas(
    free_segments: &[(i64, i64)],
    memory_regions_sizes: &[i64],
    config: &str,
    net: &str,
) -> io::Result<String> {
    

    let total_needed_memory: i64 = memory_regions_sizes.iter().sum();

    for &(start, end) in free_segments {
        if (end - start + 1) >= total_needed_memory {
            return generate_configuration((start, end), memory_regions_sizes, config, net);
        }
    }

    println!("Error: No memory area is big enough to contain the total needed memory.");
    std::process::exit(1);
}

// Function to generate the configuration
fn generate_configuration(
    free_segment: (i64, i64),
    memory_regions_sizes: &[i64],
    config: &str,
    net: &str,
) -> io::Result<String> {
    let mut logfile = OpenOptions::new()
    .create(true)
    .append(true)
    .open("/usr/share/runPHI/log_mem.txt")?;

    writeln!(logfile, "generating conf")?; //DEBUG

    let mut current_phys_start = free_segment.0;
    let lines: Vec<&str> = config.lines().collect();
    let mut updated_config = Vec::new();
    let mut size_index = 0;
    let mut skip_phys_start = false;

    for line in &lines {
        let trimmed_line = line.trim();
        if trimmed_line.starts_with("/* UART */")
            || trimmed_line.starts_with("/* communication region */")
            || trimmed_line.starts_with("/* IVSHMEM shared memory region (demo) */")
        {
            skip_phys_start = true;
        } else if trimmed_line.starts_with("/*") {
            skip_phys_start = false;
        }

        if trimmed_line.contains(".phys_start")
            && !skip_phys_start
            && size_index < memory_regions_sizes.len()
        {
            let new_phys_start_line =
                format!("            .phys_start = 0x{:08x},", current_phys_start);
            updated_config.push(new_phys_start_line);
            current_phys_start += memory_regions_sizes[size_index];
            size_index += 1;
        } else {
            updated_config.push(line.to_string());
        }
    }
    writeln!(logfile, "finished for")?; //DEBUG

    let mut output_config = updated_config.join("\n");

    // Calculate and print the end address
    let end_address = current_phys_start;

    // Update the IVSHMEM start address in output_config
    // Call update_ivshmem only if c.net is not "none"
    if net != "none" {
        writeln!(logfile, "updating ivshmeme")?; //DEBUG
        update_ivshmem(&mut output_config).expect("Failed to update IVSHMEM start address");
        writeln!(logfile, "after update ivshmeme")?; //DEBUG
    }

    // Update free_segments.txt
    update_free_segments(free_segment, end_address).expect("Failed to update free_segments.txt");

    writeln!(logfile, "generated conf")?; //DEBUG
    Ok(output_config)
}

fn update_free_segments(free_segment: (i64, i64), end_address: i64) -> io::Result<()> {
    //let file_path = "free_segments.txt"; //CHANGED TO NEW VERSION BELOW

    let file_name = "free_segments.txt";
    let file_path_string = format!("{}/{}", WORKPATH, file_name);
    let file_path: &str = &file_path_string;

    let mut free_segments = Vec::new();

    // Read the existing free_segments
    read_free_segments(file_path, &mut free_segments)?;

    // Find the segment to update
    for segment in &mut free_segments {
        if *segment == free_segment {
            segment.0 = end_address; // Update the start of the chosen free_segment
            break;
        }
    }

    // Debug print to check the updated state of free_segments

    // Write the updated free_segments back to the file
    let mut writer = File::create(file_path)?;
    for &(start, end) in &free_segments {
        writeln!(writer, "0x{:x}, 0x{:x}", start, end)?;
    }

    Ok(())
}

/* fn read_start_shmem(file_path: &str) -> io::Result<i64> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);
    let mut first_line = String::new();
    reader.read_line(&mut first_line)?;
    let value = i64::from_str_radix(first_line.trim().trim_start_matches("0x"), 16)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(value)
} */

/* fn update_start_shmem(file_path: &str, new_value: i64) -> io::Result<()> {
    let mut writer = File::create(file_path)?;
    writeln!(writer, "0x{:x}", new_value)?;
    Ok(())
} */

//TODO: dunno what is going on here, but this function is a shitbox of hardcoded stuff, fix it -- Marco
fn update_ivshmem(output_config: &mut String) -> io::Result<()> {
    let mut start_address_ivshmem: i64 = 0x7f900000;

    let pci_ivshmem_id_path = Path::new(WORKPATH).join(PCI_IVSHMEM_ID_FILE);

    let mut occupied_ids = vec![];

    if !pci_ivshmem_id_path.exists() || is_path_empty(&pci_ivshmem_id_path)? {
        //DO NOTHING AND SKIP ELSE BLOCK
    } else {
        let file = File::open(&pci_ivshmem_id_path)?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line?;
            if let Ok(id) = line.trim().parse::<i64>() {
                occupied_ids.push(id);
            }
        }
        start_address_ivshmem = if !occupied_ids.contains(&1) {
            0x7f900000
        } else if !occupied_ids.contains(&2) {
            0x7fa00000
        }
        //else if !occupied_ids.contains(&3) {
        //0x7fb00000
        //}
        else {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "No available ivshmem device",
            ));
        };
    }

    /* // Read or initialize start_shmem.txt
    if Path::new(file_path_shmem).exists() && !is_file_empty(file_path_shmem)? {
        start_address_ivshmem = read_start_shmem(file_path_shmem)?;
    } */

    // Update output_config with start_address_ivshmem
    let updated_config = output_config.replace(
        "JAILHOUSE_SHMEM_NET_REGIONS(0x7f900000, 1),",
        &format!(
            "JAILHOUSE_SHMEM_NET_REGIONS(0x{:08x}, 1),",
            start_address_ivshmem
        ),
    );

    *output_config = updated_config;

    // Save the updated start_address_ivshmem to start_shmem.txt
    // update_start_shmem(file_path_shmem, start_address_ivshmem + 0x100000)?;

    Ok(())
}

//End my functions

pub fn memconfig(
    c: &mut configGenerator::Backendconfig,
    mem_request_hex: &str,
) -> Result<(), Box<dyn Error>> {

    let mut logfile = OpenOptions::new()
    .create(true)
    .append(true)
    .open("/usr/share/runPHI/log_mem.txt")?;

    writeln!(logfile, "First line of memconf")?; //DEBUG

    let config_template_with_net = r#"
    .mem_regions = {
    /* IVSHMEM shared memory region (demo) */ {
        .phys_start = 0x7f8f0000,
        .virt_start = 0x7f8f0000,
        .size = 0x1000,
        .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_ROOTSHARED,
    },
    /* IVSHMEM shared memory region (demo) */ {
        .phys_start = 0x7f8f1000,
        .virt_start = 0x7f8f1000,
        .size = 0x9000,
        .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
            JAILHOUSE_MEM_ROOTSHARED,
    },
    /* IVSHMEM shared memory region (demo) */ {
        .phys_start = 0x7f8fa000,
        .virt_start = 0x7f8fa000,
        .size = 0x2000,
        .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_ROOTSHARED,
    },
    /* IVSHMEM shared memory region (demo) */ {
        .phys_start = 0x7f8fc000,
        .virt_start = 0x7f8fc000,
        .size = 0x2000,
        .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_ROOTSHARED,
    },
    /* IVSHMEM shared memory region (demo) */ {
        .phys_start = 0x7f8fe000,
        .virt_start = 0x7f8fe000,
        .size = 0x2000,
        .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
            JAILHOUSE_MEM_ROOTSHARED,
    },
    /* IVSHMEM shared memory region (networking) */
    JAILHOUSE_SHMEM_NET_REGIONS(0x7f900000, 1),
    /* UART */ {
        .phys_start = 0x09000000,
        .virt_start = 0x09000000,
        .size = 0x1000,
        .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
            JAILHOUSE_MEM_IO | JAILHOUSE_MEM_ROOTSHARED,
    },
    /* RAM */ {
        .phys_start = 0x7e900000,
        .virt_start = 0,
        .size = 0x10000,
        .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
            JAILHOUSE_MEM_EXECUTE | JAILHOUSE_MEM_LOADABLE,
    },
    /* RAM */ {
        .phys_start = 0x70000000,
        .virt_start = 0x70000000,
        .size = {requested_size},
        .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
            JAILHOUSE_MEM_EXECUTE | JAILHOUSE_MEM_DMA |
            JAILHOUSE_MEM_LOADABLE,
    },
    /* communication region */ {
        .virt_start = 0x80000000,
        .size = 0x00001000,
        .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
            JAILHOUSE_MEM_COMM_REGION,
    },
},
"#;

    let config_template_without_net = r#"
    .mem_regions = {
    /* UART */ {
        .phys_start = 0x09000000,
        .virt_start = 0x09000000,
        .size = 0x1000,
        .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
            JAILHOUSE_MEM_IO | JAILHOUSE_MEM_ROOTSHARED,
    },
    /* RAM */ {
        .phys_start = 0x7e900000,
        .virt_start = 0,
        .size = 0x10000,
        .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
            JAILHOUSE_MEM_EXECUTE | JAILHOUSE_MEM_LOADABLE,
    },
    /* RAM */ {
        .phys_start = 0x70000000,
        .virt_start = 0x70000000,
        .size = {requested_size},
        .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
            JAILHOUSE_MEM_EXECUTE | JAILHOUSE_MEM_DMA |
            JAILHOUSE_MEM_LOADABLE,
    },
    /* communication region */ {
        .virt_start = 0x80000000,
        .size = 0x00001000,
        .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
            JAILHOUSE_MEM_COMM_REGION,
    },
},
"#;

    // Choose the appropriate template based on c.net
    let config_template = if c.net == "none" {
        config_template_without_net
    } else {
        config_template_with_net
    };

    // Replace value and explicitly convert the String to &str
    let config: &str = &config_template.replace("{requested_size}", mem_request_hex);

    // Insert line into the config file
    let pattern = r"struct jailhouse_cell_desc cell;\n[ \t]*__u64 cpus\[\d*\];";
    let pattern_fpga = r"[\t]*__u64 fpga_regions\[\d*\];";
    let num_regions = count_mem_regions(config);
    let linetoinsert = format!("\tstruct jailhouse_memory mem_regions[{}];", num_regions); // Use format! to include the variable

    // Compile a regular expression to match the pattern and insert the cpus
    let re = Regex::new(&pattern)?;
    if let Some(pos) = re.find(&c.conf) {
        let re = Regex::new(&pattern_fpga)?;
        let old_pos = pos.clone();
        if let Some(pos) = re.find(&c.conf) {
            c.conf
            .insert_str(pos.end(), &format!("\n{}\n", linetoinsert));}
        else{
        c.conf
            .insert_str(old_pos.end(), &format!("\n{}\n", linetoinsert));}
    } else {
        return Err("\"struct jailhouse_cell_desc cell\" not found".into());
    }

    writeln!(logfile, "memconf after regex")?; //DEBUG

    let free_segments_file_name = "free_segments.txt";
    let free_segments_file_path_string = format!("{}/{}", WORKPATH, free_segments_file_name);
    let free_segments_file_path: &str = &free_segments_file_path_string;

    //NOTE THAT AVAILABLE MEMORY SHOULDN'T BE HARDCODED BUT IT SHOULD READ A FILE BOARD DEPENDENT
    //MANAGED BY THE BOARD AND NOT BY US
    let available_memory_file_name = "available_memory.txt";
    let available_memory_file_path_string = format!("{}/{}", WORKPATH, available_memory_file_name);
    let available_memory_file_path: &str = &available_memory_file_path_string;

    /* let shmem_file_name = "start_shmem.txt";
    let shmem_file_path_string = format!("{}/{}", WORKPATH, shmem_file_name);
    let file_path_shmem: &str = &shmem_file_path_string; */

    let _ivshmem_size = 0xff000; // IVSHMEM size in hexadecimal
    let mut memory_regions_sizes: Vec<i64> = Vec::new(); // Memory regions sizes vector

    // Populate memory_regions_sizes by extracting .size fields from the config string
    populate_memory_regions_sizes(config, &mut memory_regions_sizes);

    writeln!(logfile, "memconf after populate_mem")?; //DEBUG

    let mut free_segments = Vec::new();

    // Create or open the start_shmem.txt file if it doesn't exist
    /*     let _start_shmem_file = OpenOptions::new()
    .write(true)
    .create(true)
    .open(file_path_shmem)?; */

    writeln!(logfile, "memconf after shmem_file")?; //DEBUG

    // Check if free_segments.txt doesn't exist or is empty
    if !Path::new(free_segments_file_path).exists() || is_file_empty(free_segments_file_path)? {
        // Populate free_segments by reading from available_memory.txt
        obtain_memory_space(available_memory_file_path, &mut free_segments)?;
        writeln!(logfile, "obtained free mem")?; //DEBUG

        // Initialize the free_segments.txt file with the obtained free_segments
        initialize_free_segments(free_segments_file_path, &free_segments)?;
        writeln!(logfile, "initialized free seg")?; //DEBUG
    } else {
        // Read segments from free_segments.txt if it already exists
        read_free_segments(free_segments_file_path, &mut free_segments)?;
        writeln!(logfile, "memconf after read_free_segments")?; //DEBUG
    }
    writeln!(logfile, "memconf after free_segments_part")?; //DEBUG

    // Check if at least one of the memory areas in free_segments is big enough
    let output_config = check_memory_areas(&free_segments, &memory_regions_sizes, config, &c.net)?;

    writeln!(logfile, "configuration file: {}", output_config)?; //DEBUG

    c.conf.push_str(&output_config);

    writeln!(logfile, "last line of memconf")?; //DEBUG

    Ok(())
}
