//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//*********************************************

use serde_json;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::process::{exit, Command};

// This file implements all the logic related to the forwarding to runc

// forward to runc if the filter detects the need
// THE FUNCTION MAY EXIT THE PROGRAM
// This function always has the config in the json structure, however this is non empty only
// for create case.
//TODO: improve code quality here, do not manage program exit inside function
pub fn runc_forward_ifnecessary(config: &serde_json::Value, containerid: &str) {
    if need_forward_to_runc(&config, &containerid) {
        logging::log_message(logging::Level::Info,  format!("Forwarding to runc id {}", &containerid).as_str());
        call_runc()
    }
}

pub fn call_runc() {
    let mut runccmd = Command::new("/usr/local/sbin/runc_vanilla");
    for arg in std::env::args().skip(1) {
        runccmd.arg(arg);
    }
    match runccmd.status() {
        Ok(status) => {
            if status.success() {
                exit(0);
            } else {
                exit(1);
            }
        }
        Err(_) => {
            exit(1);
        }
    }
}

pub fn runc_forward_ifnecessary_delete(config: &serde_json::Value, containerid: &str) {
    if need_forward_to_runc(&config, &containerid) {
        logging::log_message(logging::Level::Info,  format!("Forwarding to runc id {}", &containerid).as_str());
        let mut runccmd = Command::new("/usr/local/sbin/runc_vanilla");
        for arg in std::env::args().skip(1) {
            runccmd.arg(arg);
        }
        match runccmd.status() {
            Ok(status) => {
                if status.success() {
                    delete_entry_table(&containerid);
                    exit(0);
                } else {
                    logging::log_message(logging::Level::Error, "Runc returned an error");
                    exit(1);
                }
            }
            Err(_) => {
                logging::log_message(logging::Level::Error, "Runc returned an error");
                exit(1);
            }
        }
    }
}

// Recognize if to forward to runc
// Here the problem is that we do not really create a pause contaienr, as default in kubernetes
// The forwarding is also needed for monitoring daemons that can run on the node
// Hence a more complex parsing may be needed to specify the runtime of DaemonSets (in upper layers??)
pub fn need_forward_to_runc(config: &serde_json::Value, containerid: &str) -> bool {
    // Check if `config.process.args` exists and is an array
    if let Some(args) = config
        .pointer("/process/args")
        .and_then(serde_json::Value::as_array)
    {
        for arg in args {
            // Check if the element is "/pause" to identify pause containers
            if let Some(pause) = arg.as_str() {
                if pause == "/pause" {
                    let mut redirect_table = OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open("/usr/share/runPHI/redirect.txt")
                        .expect("Failed to open in append table file");
                    let _ = writeln!(redirect_table, "{}", &containerid);
                    return true;
                }
            }
        }
    }
    // Otherwise, check if the id is in the table.
    // If it fails to open file, return false (there was no filter to store any ID in the table)
    if let Ok(lines) = read_lines("/usr/share/runPHI/redirect.txt") {
        // Consumes the iterator, returns an (Optional) String
        for line in lines.flatten() {
            if line.contains(containerid) {
                return true;
            }
        }
    }
    return false;
}

// This function, if needed updates the forwarding table removing the id of removed container
//TODO: parametrize file paths
fn delete_entry_table(containerid: &str) {
    let file =
        fs::File::open("/usr/share/runPHI/redirect.txt").expect("Failed to find redirect file");
    let reader = BufReader::new(file);
    let mut lines: Vec<String> = Vec::new();
    for line in reader.lines() {
        lines.push(line.expect("Failed to read line"));
    }
    // keep the string if it does not match the containerid
    lines.retain(|line| !line.contains(containerid));
    let mut redirect_table = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open("/usr/share/runPHI/redirect.txt")
        .expect("Failed to open in append table file");
    for line in lines {
        let _ = writeln!(redirect_table, "{}", line);
    }
}

// The output is wrapped in a Result to allow matching on errors.
// Returns an Iterator to the Reader of the lines of the file.
fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<fs::File>>>
where
    P: AsRef<Path>,
{
    let file = fs::File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}
