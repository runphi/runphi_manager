//*********************************************
// Authors: Marco Barletta (marco.barletta@unina.it)
//          Francesco Boccola (francesco.boccola@unina.it)
//*********************************************

use std::path::{Path, PathBuf};
use std::process::Command;

use f2b::paths;

// OCI annotation that explicitly selects the low-level runtime, overriding
// auto-detection. Recognized values: "runphi" or "runc".
const RUNTIME_ANNOTATION: &str = "org.runphi.runtime";

// Outcome of a forwarding check. The caller (main) decides what to do
// (exec runc and exit, or continue with the runPHI flow), keeping
// process-control out of this module.
pub enum ForwardDecision {
    UseRunphi,
    ForwardToRunc,
}

// Decide whether the OCI `create` for the given container must be forwarded
// to vanilla runc. Selection rules, in order:
//   1. Explicit annotation `org.runphi.runtime` ("runphi" or "runc").
//   2. Auto-detect: a container is treated as runPHI-managed iff its rootfs
//      contains `/boot/config.json` (the boot parameters for the partitioned
//      cell). Anything else - including the Kubernetes pause container and
//      arbitrary docker images - is forwarded to runc.
pub fn decide_create(config: &serde_json::Value, bundle: &Path) -> ForwardDecision {
    if let Some(runtime) = config.pointer(&format!("/annotations/{}", RUNTIME_ANNOTATION)).and_then(serde_json::Value::as_str) {
        match runtime {
            "runphi" => return ForwardDecision::UseRunphi,
            "runc" => return ForwardDecision::ForwardToRunc,
            other => logging::log_message(
                logging::Level::Warn,
                format!(
                    "Unknown {}=\"{}\", falling back to auto-detect",
                    RUNTIME_ANNOTATION, other
                )
                .as_str(),
            ),
        }
    }

    // Auto-detect
    if rootfs_has_runphi_boot_config(config, bundle) {
        ForwardDecision::UseRunphi
    } else {
        ForwardDecision::ForwardToRunc
    }
}

// Decide whether an OCI command other than `create` must be forwarded.
// A container is runPHI-managed iff its state directory still exists.
pub fn decide_existing(containerid: &str) -> ForwardDecision {
    if Path::new(paths::STATE_DIR).join(containerid).exists() {
        ForwardDecision::UseRunphi
    } else {
        ForwardDecision::ForwardToRunc
    }
}

// Exec vanilla runc with the same arguments as the current invocation
// and return its exit code (defaults to 1 if runc cannot be launched).
pub fn call_runc() -> i32 {
    let forwarded_args: Vec<String> = std::env::args().skip(1).collect();
    logging::log_message(
        logging::Level::Trace,
        format!(
            "Redirecting to runc: {} {}",
            paths::RUNC_VANILLA_BIN,
            forwarded_args.join(" ")
        )
        .as_str(),
    );
    let mut runccmd = Command::new(paths::RUNC_VANILLA_BIN);
    for arg in &forwarded_args {
        runccmd.arg(arg);
    }
    match runccmd.status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(e) => {
            logging::log_message(
                logging::Level::Error,
                format!("Failed to exec {}: {}", paths::RUNC_VANILLA_BIN, e).as_str(),
            );
            1
        }
    }
}

fn rootfs_has_runphi_boot_config(config: &serde_json::Value, bundle: &Path) -> bool {
    let rootfs = match config
        .pointer("/root/path")
        .and_then(serde_json::Value::as_str)
    {
        Some(p) => p,
        None => return false,
    };

    let rootfs_path: PathBuf = if Path::new(rootfs).is_absolute() {
        PathBuf::from(rootfs)
    } else {
        bundle.join(rootfs)
    };

    rootfs_path.join(paths::BOOT_CONFIG_REL).exists()
}
