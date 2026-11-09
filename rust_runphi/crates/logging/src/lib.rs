//*********************************************
// Authors: 
// Marco Barletta (marco.barletta@unina.it)
// Francesco Tafuri (fran.tafuri@studenti.unina.it)
//*********************************************

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::env;

use lazy_static::lazy_static;
pub use log::Level;

pub mod timer;

/// Default log file path. The timer module writes raw tick-stamped
/// phase markers to the same file.
pub const LOG_PATH: &str = "/usr/share/runPHI/log.txt";

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
        let effective_path = path.unwrap_or_else(|| PathBuf::from(LOG_PATH));

        // Ensure the parent directory exists. create(true) only creates the
        // file, not intermediate directories, so on a fresh host (e.g. an x86
        // Xen Dom0 where /usr/share/runPHI does not exist yet) the open would
        // otherwise panic with NotFound on the very first invocation.
        if let Some(parent) = effective_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // The log is a debugging aid: a file runphi cannot open (no permission,
        // a read-only /usr, a directory it could not create) must not stop it
        // from managing containers, so it then runs without one, and says so
        // once on stderr (which callers such as the containerd shim do not
        // parse, unlike stdout).
        let file = match OpenOptions::new()
                .create(true)
                .append(true)
                .open(&effective_path) {
            Ok(file) => Some(file),
            Err(e) => {
                warn_on_stderr(&format!(
                    "cannot open log file {}: {}; logging disabled",
                    effective_path.display(),
                    e
                ));
                None
            }
        };

        Logger { level, file, path: Some(effective_path) }
    }

    // Log method, writes to the file if provided
    fn log(&mut self, level: Level, message: &str) {
        if level <= self.level {
            let log_entry = format!("{:?}: {}\n", level, message);

            // Log to file if file handler is available. A write that fails
            // (e.g. a full disk) disables the file instead of panicking.
            if let Some(file) = self.file.as_mut() {
                if let Err(e) = file.write_all(log_entry.as_bytes()) {
                    warn_on_stderr(&format!(
                        "cannot write to log file {}: {}; logging disabled",
                        self.path.as_deref().unwrap_or(Path::new("")).display(),
                        e
                    ));
                    self.file = None;
                }
            }

            // Optionally, print to the console (for demo purposes)
            // print!("{}", log_entry);
        }
    }

    // Method to change the log level
    pub fn set_level(&mut self, level: Level) {
        self.level = level;
    }
}

// A warning about the log itself. Ignores a failing stderr, which eprintln!
// would turn into a panic.
fn warn_on_stderr(message: &str) {
    let _ = writeln!(std::io::stderr(), "runphi: warning: {}", message);
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
    let level_str = env::var("RUNPHI_DEBUG_LEVEL").unwrap_or_else(|_| "info".to_string());        
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

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("runphi-logging-{}-{}", std::process::id(), name))
    }

    #[test]
    fn logs_to_the_file() {
        let path = scratch_path("ok.txt");
        let mut logger = Logger::new(Level::Info, Some(path.clone()));
        logger.log(Level::Info, "kept");
        logger.log(Level::Debug, "below the level");
        let content = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(content, "Info: kept\n");
    }

    // A log file that cannot be opened (here its parent is a regular file, so
    // the directory cannot be created either) leaves runphi without a log
    // instead of panicking.
    #[test]
    fn unopenable_log_file_disables_logging() {
        let parent = scratch_path("not-a-dir");
        std::fs::write(&parent, "").unwrap();
        let mut logger = Logger::new(Level::Info, Some(parent.join("log.txt")));
        std::fs::remove_file(&parent).unwrap();
        assert!(logger.file.is_none());
        logger.log(Level::Error, "dropped");
    }

    // A write that fails (/dev/full: ENOSPC) disables the file.
    #[cfg(target_os = "linux")]
    #[test]
    fn failing_write_disables_logging() {
        let mut logger = Logger::new(Level::Info, Some(PathBuf::from("/dev/full")));
        assert!(logger.file.is_some());
        logger.log(Level::Info, "no space");
        assert!(logger.file.is_none());
        logger.log(Level::Info, "not tried again");
    }
}
