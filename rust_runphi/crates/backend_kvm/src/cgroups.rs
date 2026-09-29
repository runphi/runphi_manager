use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use libcgroups::common::{
    create_cgroup_manager, CgroupConfig, CgroupManager, ControllerOpt, FreezerState,
};
use libcgroups::stats::Stats;
use nix::unistd::Pid;
use oci_spec::runtime::{LinuxMemoryBuilder, LinuxResources};

const CGROUP_PATH_FILE: &str = "cgroup_path";
const SYSTEMD_CGROUP_FILE: &str = "systemd_cgroup";

/// Resolves the cgroup path from OCI configuration or defaults to "runphi/<container_id>".
pub fn resolve_cgroup_path(fc: &f2b::FrontendConfig) -> PathBuf {
    if let Some(cgroups_path) = fc.jsonconfig["linux"]["cgroupsPath"].as_str() {
        if !cgroups_path.trim().is_empty() {
            let path = Path::new(cgroups_path);
            let path = if path.is_absolute() {
                path.strip_prefix("/").unwrap_or(path)
            } else {
                path
            };
            return path.to_path_buf();
        }
    }
    PathBuf::from(format!("runphi/{}", fc.containerid))
}

/// Creates a CgroupConfig from runtime parameters.
fn build_cgroup_config(
    cgroup_path: PathBuf,
    container_id: &str,
    systemd_cgroup: bool,
) -> CgroupConfig {
    CgroupConfig {
        cgroup_path,
        systemd_cgroup,
        container_name: container_id.to_string(),
    }
}

/// Builds the LinuxResources to enforce, injecting image memory fallback if needed.
fn build_linux_resources(
    fc: &f2b::FrontendConfig,
    ic: &f2b::ImageConfig,
) -> Result<LinuxResources, Box<dyn Error>> {
    let mut resources: LinuxResources = if let Some(res_val) =
        fc.jsonconfig.get("linux").and_then(|l| l.get("resources"))
    {
        serde_json::from_value(res_val.clone()).unwrap_or_default()
    } else {
        LinuxResources::default()
    };

    // If ic.memory is explicitly set and no OCI memory limit was defined, inject it
    if ic.memory > 0 && resources.memory().is_none() {
        let mem = LinuxMemoryBuilder::default()
            .limit((ic.memory * 1024 * 1024) as i64)
            .build()?;
        resources.set_memory(Some(mem));
    }

    Ok(resources)
}

/// Sets up the cgroup for the QEMU guest process, attaches the process PID, and applies resource limits.
pub fn setup_cgroups(
    fc: &f2b::FrontendConfig,
    ic: &f2b::ImageConfig,
    pid: u32,
) -> Result<(), Box<dyn Error>> {
    let cgroup_path = resolve_cgroup_path(fc);
    let systemd_cgroup = fc.jsonconfig["systemd_cgroup"]
        .as_bool()
        .unwrap_or(false);

    // Save the cgroup path and driver in crundir for subsequent lifecycle commands (e.g., destroy/freeze/stats)
    let cgroup_path_file = fc.crundir.join(CGROUP_PATH_FILE);
    fs::write(&cgroup_path_file, cgroup_path.to_string_lossy().as_bytes())?;

    let systemd_cgroup_file = fc.crundir.join(SYSTEMD_CGROUP_FILE);
    fs::write(
        &systemd_cgroup_file,
        if systemd_cgroup { b"1" } else { b"0" },
    )?;

    let config = build_cgroup_config(cgroup_path.clone(), &fc.containerid, systemd_cgroup);
    let manager = create_cgroup_manager(config)
        .map_err(|e| format!("Failed to create cgroup manager: {}", e))?;

    // Move the QEMU main PID into the cgroup. All sub-threads (vCPUs, I/O threads) inherit this cgroup.
    let nix_pid = Pid::from_raw(pid as i32);
    manager
        .add_task(nix_pid)
        .map_err(|e| format!("Failed to add task (PID {}) to cgroup: {}", pid, e))?;

    let resources = build_linux_resources(fc, ic)?;

    let oom_score_adj = fc.jsonconfig["process"]["oomScoreAdj"]
        .as_i64()
        .map(|v| v as i32);

    let controller_opt = ControllerOpt {
        resources: &resources,
        disable_oom_killer: false,
        oom_score_adj,
        freezer_state: None,
    };

    manager
        .apply(&controller_opt)
        .map_err(|e| format!("Failed to apply cgroup resource limits: {}", e))?;

    logging::log_message(
        logging::Level::Info,
        &format!(
            "Cgroup setup completed for container {} (PID {}) at {:?}",
            fc.containerid, pid, cgroup_path
        ),
    );

    Ok(())
}

/// Deletes the cgroup directory when the guest is destroyed.
pub fn destroy_cgroups(containerid: &str, crundir: &Path) -> Result<(), Box<dyn Error>> {
    let cgroup_path = get_stored_cgroup_path(containerid, crundir);
    let systemd_cgroup = get_stored_systemd_cgroup(crundir);
    let config = build_cgroup_config(cgroup_path.clone(), containerid, systemd_cgroup);

    if let Ok(manager) = create_cgroup_manager(config) {
        if let Err(e) = manager.remove() {
            logging::log_message(
                logging::Level::Warn,
                &format!("Could not remove cgroup {:?}: {}", cgroup_path, e),
            );
        } else {
            logging::log_message(
                logging::Level::Info,
                &format!("Removed cgroup {:?}", cgroup_path),
            );
        }
    }

    Ok(())
}

/// Freezes or resumes all processes inside the cgroup.
pub fn freeze_cgroups(
    containerid: &str,
    crundir: &Path,
    freeze: bool,
) -> Result<(), Box<dyn Error>> {
    let cgroup_path = get_stored_cgroup_path(containerid, crundir);
    let systemd_cgroup = get_stored_systemd_cgroup(crundir);
    let config = build_cgroup_config(cgroup_path, containerid, systemd_cgroup);

    let manager = create_cgroup_manager(config)
        .map_err(|e| format!("Failed to create cgroup manager for freeze: {}", e))?;

    let state = if freeze {
        FreezerState::Frozen
    } else {
        FreezerState::Thawed
    };

    manager
        .freeze(state)
        .map_err(|e| format!("Failed to set freezer state to {:?}: {}", state, e))?;

    Ok(())
}

/// Retrieves resource utilization statistics for the cgroup.
pub fn get_cgroup_stats(containerid: &str, crundir: &Path) -> Result<Stats, Box<dyn Error>> {
    let cgroup_path = get_stored_cgroup_path(containerid, crundir);
    let systemd_cgroup = get_stored_systemd_cgroup(crundir);
    let config = build_cgroup_config(cgroup_path, containerid, systemd_cgroup);

    let manager = create_cgroup_manager(config)
        .map_err(|e| format!("Failed to create cgroup manager for stats: {}", e))?;

    manager
        .stats()
        .map_err(|e| format!("Failed to read cgroup stats: {}", e).into())
}

/// Reads the saved cgroup path from crundir or falls back to "runphi/<container_id>".
fn get_stored_cgroup_path(containerid: &str, crundir: &Path) -> PathBuf {
    let cgroup_path_file = crundir.join(CGROUP_PATH_FILE);
    if let Ok(content) = fs::read_to_string(&cgroup_path_file) {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    PathBuf::from(format!("runphi/{}", containerid))
}

/// Reads the saved systemd_cgroup boolean from crundir (defaults to false).
fn get_stored_systemd_cgroup(crundir: &Path) -> bool {
    let systemd_cgroup_file = crundir.join(SYSTEMD_CGROUP_FILE);
    if let Ok(content) = fs::read_to_string(&systemd_cgroup_file) {
        return content.trim() == "1" || content.trim() == "true";
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_cgroup_path_default() {
        let mut fc = f2b::FrontendConfig::new();
        fc.containerid = "test-cont-123".to_string();
        fc.jsonconfig = serde_json::json!({});

        let path = resolve_cgroup_path(&fc);
        assert_eq!(path, PathBuf::from("runphi/test-cont-123"));
    }

    #[test]
    fn test_resolve_cgroup_path_custom_relative() {
        let mut fc = f2b::FrontendConfig::new();
        fc.containerid = "test-cont-123".to_string();
        fc.jsonconfig = serde_json::json!({
            "linux": {
                "cgroupsPath": "docker/custom-cgroup"
            }
        });

        let path = resolve_cgroup_path(&fc);
        assert_eq!(path, PathBuf::from("docker/custom-cgroup"));
    }

    #[test]
    fn test_resolve_cgroup_path_custom_absolute() {
        let mut fc = f2b::FrontendConfig::new();
        fc.containerid = "test-cont-123".to_string();
        fc.jsonconfig = serde_json::json!({
            "linux": {
                "cgroupsPath": "/system.slice/runphi-test.scope"
            }
        });

        let path = resolve_cgroup_path(&fc);
        assert_eq!(path, PathBuf::from("system.slice/runphi-test.scope"));
    }

    #[test]
    fn test_resolve_cgroup_path_empty_or_whitespace() {
        let mut fc = f2b::FrontendConfig::new();
        fc.containerid = "test-cont-123".to_string();
        fc.jsonconfig = serde_json::json!({
            "linux": {
                "cgroupsPath": "   "
            }
        });

        let path = resolve_cgroup_path(&fc);
        assert_eq!(path, PathBuf::from("runphi/test-cont-123"));
    }

    #[test]
    fn test_stored_cgroup_path_and_systemd() {
        let temp_dir = std::env::temp_dir().join(format!("test_runphi_{}", std::process::id()));
        fs::create_dir_all(&temp_dir).unwrap();

        // When files do not exist:
        assert_eq!(
            get_stored_cgroup_path("mycont", &temp_dir),
            PathBuf::from("runphi/mycont")
        );
        assert!(!get_stored_systemd_cgroup(&temp_dir));

        // Write files
        fs::write(temp_dir.join(CGROUP_PATH_FILE), "docker/mycont\n").unwrap();
        fs::write(temp_dir.join(SYSTEMD_CGROUP_FILE), "1\n").unwrap();

        assert_eq!(
            get_stored_cgroup_path("mycont", &temp_dir),
            PathBuf::from("docker/mycont")
        );
        assert!(get_stored_systemd_cgroup(&temp_dir));

        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_build_linux_resources_memory_fallback() {
        let mut fc = f2b::FrontendConfig::new();
        fc.jsonconfig = serde_json::json!({
            "linux": {
                "resources": {
                    "cpu": {
                        "quota": 100000
                    }
                }
            }
        });

        let ic: f2b::ImageConfig = serde_json::from_value(serde_json::json!({
            "memory": 512
        })).unwrap();

        let res = build_linux_resources(&fc, &ic).unwrap();
        assert!(res.memory().is_some());
        assert_eq!(
            res.memory().as_ref().unwrap().limit(),
            Some(512 * 1024 * 1024)
        );
        assert!(res.cpu().is_some());
        assert_eq!(res.cpu().as_ref().unwrap().quota(), Some(100000));
    }

    #[test]
    fn test_build_linux_resources_explicit_memory_preserved() {
        let mut fc = f2b::FrontendConfig::new();
        fc.jsonconfig = serde_json::json!({
            "linux": {
                "resources": {
                    "memory": {
                        "limit": 1073741824
                    }
                }
            }
        });

        let ic: f2b::ImageConfig = serde_json::from_value(serde_json::json!({
            "memory": 256
        })).unwrap();

        let res = build_linux_resources(&fc, &ic).unwrap();
        assert_eq!(
            res.memory().as_ref().unwrap().limit(),
            Some(1073741824)
        );
    }
}
