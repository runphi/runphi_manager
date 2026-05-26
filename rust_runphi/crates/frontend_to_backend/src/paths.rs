//*********************************************
// Authors:
// Francesco Boccola (francesco.boccola@unina.it)
//*********************************************

//! Centralized filesystem paths used by runPHI.
//!
//! Cross-cutting paths live here so the runphi binary, the active
//! backend, and the shared crates all reference one source of truth.
//! Per-backend or per-platform paths (e.g. JAILHOUSE_PATH, /dev/mem)
//! stay in the backend crate that owns them, since they describe
//! that backend's environment.

// --- Host directories owned by runPHI ---

/// Base directory for runPHI's host-side state and helpers.
/// Other runphi-owned host paths are placed under here.
pub const WORKPATH: &str = "/usr/share/runPHI";

/// Parent of per-container state directories. Each container lives
/// in `<STATE_DIR>/<containerid>/`. Presence of that subdirectory is
/// the source of truth for "this container is runPHI-managed" for
/// OCI commands other than `create`.
pub const STATE_DIR: &str = "/run/runPHI";

// --- Container rootfs (paths relative to the mounted rootfs) ---

/// Boot parameters for the partitioned cell. Presence of this file
/// in the rootfs is the runPHI image marker at `create` time.
pub const BOOT_CONFIG_REL: &str = "boot/config.json";

/// Default inmate binary path used when BOOT_CONFIG_REL does not
/// specify one explicitly.
pub const BOOT_INMATE_DEFAULT_REL: &str = "boot/boot.bin";

// --- External tools runPHI invokes ---

/// Distro-provided runc, backed up here by switch_to_runphi.sh.
/// Used when forwarding non-runPHI containers.
pub const RUNC_VANILLA_BIN: &str = "/usr/local/sbin/runc_vanilla";

/// Caronte helper that keeps a process alive for containerd while
/// the hypervisor manages the actual workload.
pub const CARONTE_BIN: &str = "/usr/share/runPHI/caronte";
