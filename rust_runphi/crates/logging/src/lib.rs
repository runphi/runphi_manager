//*********************************************
// Authors: 
// Marco Barletta (marco.barletta@unina.it)
// Francesco Tafuri (fran.tafuri@studenti.unina.it)
//*********************************************

// use serde::Deserialize;
// use serde_json;
// use std::fs;

// use std::path::PathBuf;
// use std::path::Path;

//This method is used to write log messages on the json file 
// pub fn write_to_json(msg: &String, log_path: &Path)->Result<(),Box<dyn Error>>{
//     let mut file = OpenOptions::new()
//     .create(true)
//     .read(true)
//     .write(true)
//     .append(true)
//     .open(log_path)?;

//     let mut f_content = String::new();
//     //Read the content of the given file 
//     file.read_to_string(&mut f_content)?; 
//     //if the file is empty 
//     let mut json_array:Vec<Value>=if f_content.is_empty(){
//         Vec::new() //create a new array
//     }else{
//         serde_json::from_str(&f_content)? //deserialize the content read from the file 
//     };
//     //Create a new message with the content of the given msg
//     let message = Message {content:msg.clone(),};
//     //Serialize the message in a JSON Value and puth it into the array 
//     let message_json = serde_json::to_value(&message)?;
//     json_array.push(message_json);
//     //Serialize the array with the updated content 
//     let updated_content = serde_json::to_string_pretty(&json_array)?;
//     file.seek(SeekFrom::Start(0))?;
//     //Clear the file 
//     let _ = file.set_len(0);
//     //Write on the file
//     let _ = file.write_all(updated_content.as_bytes());
//     Ok(())
// }

// pub fn get_logfile(log: Option<PathBuf>)->String {
//     //OBTAIN LOG FILE PATH
//     let logfile = match &log{
//         Some(log_path)=> log_path.as_path(),
//         None => {
//             return "/dev/err"
//         },
//     };
//     return logfile;
// }
    


use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::env;

use lazy_static::lazy_static;
pub use log::Level;

// Define the Logger struct
#[derive(Debug)]
pub struct Logger {
    pub level: Level,
    file: Option<File>, // File handler if logging to a file
    pub path: Option<PathBuf>, // Optional path for the log file
}

impl Logger {
    // Constructor accepts a log level and an optional path for the log file
    fn new(level: Level, path: Option<PathBuf>) -> Self {
        let effective_path = path.unwrap_or_else(|| PathBuf::from("/usr/share/runPHI/log.txt"));

        let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&effective_path)
                .expect("Failed to open log file");

        Logger { level, file: Some(file), path: Some(effective_path) }
    }

    // Log method, writes to the file if provided
    fn log(&mut self, level: Level, message: &str) {
        if level <= self.level {
            let log_entry = format!("{:?}: {}\n", level, message);

            // Log to file if file handler is available
            if let Some(file) = self.file.as_mut() {
                file.write_all(log_entry.as_bytes()).expect("Failed to write to log file");
            }

            // Optionally, print to the console (for demo purposes)
            println!("{}", log_entry);
        }
    }

    // Method to change the log level
    pub fn set_level(&mut self, level: Level) {
        self.level = level;
    }
}

// Global logger, initialized later in `main`
lazy_static! {
    static ref LOGGER: Mutex<Option<Logger>> = Mutex::new(None);
}


// Function to convert the string representation of log level to log::Level
fn parse_log_level(level_str: &str) -> Option<Level> {
    match level_str.to_lowercase().as_str() {
        "error" => Some(Level::Error),
        "warn" => Some(Level::Warn),
        "info" => Some(Level::Info),
        "debug" => Some(Level::Debug),
        "trace" => Some(Level::Trace),
        _ => None, // Return None for invalid level strings
    }
}


// Function to initialize the logger in the global state
pub fn init_logger(path: Option<PathBuf>) {
    // Get the logging level from the environment variable
    let level_str = env::var("RUNPHI_DEBUG_LEVEL").unwrap_or_else(|_| "error".to_string());        
    // Parse the logging level from the string
    let level = parse_log_level(&level_str).unwrap_or(Level::Info); // Fallback to Info if invalid

    let logger = Logger::new(level, path);

    *LOGGER.lock().unwrap() = Some(logger);
}

// Function to log a message
pub fn log_message(level: Level, message: &str) {
    if let Some(ref mut logger) = *LOGGER.lock().unwrap() {
        logger.log(level, message);
    } else {
        eprintln!("Logger is not initialized!");
    }
}

// fn main() {
//     // Example of initializing the logger with a file path
//     let log_path = Some(PathBuf::from("log.txt"));
//     init_logger(Level::Info, log_path);

//     // Log some messages with different levels
//     log_message(Level::Info, "This is an info message.");
//     log_message(Level::Error, "This is an error message.");
//     log_message(Level::Debug, "This is a debug message."); // Won't be logged, as default level is Info

//     // Change logging level
//     {
//         let mut logger = LOGGER.lock().unwrap();
//         if let Some(ref mut logger_instance) = *logger {
//             logger_instance.set_level(Level::Debug);
//         }
//     }

//     // Log after changing the level
//     log_message(Level::Debug, "This is a debug message, after changing the log level.");
// }