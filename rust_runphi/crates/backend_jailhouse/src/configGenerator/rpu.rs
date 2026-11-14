//*********************************************
// Authors: Francesco Boccola (francesco.boccola@unina.it)
//*********************************************

use std::error::Error;
use std::fs;
use std::path::Path;
use toml::Value;

use crate::configGenerator;

// An R5 core (rCPU) of the platform, `[rcpu.<id>]` in platform_info.toml: its
// TCMs (global addresses) and its DDR region. The DDR region is the core's
// memory-region in the device tree: remoteproc can only load a program into it
// (or into the TCMs), so the programs for that core are linked there.
#[derive(Debug, Clone)]
pub struct RcpuInfo {
    pub id: i8,
    pub tcma: u64,
    pub tcmb: u64,
    pub tcm_size: u64,
    pub ddr_start: u64,
    pub ddr_size: u64,
}

fn hex_field(section: &Value, key: &str, id: &str) -> Result<u64, Box<dyn Error>> {
    let s = section
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("[rcpu.{}] in platform_info.toml has no {}", id, key))?;
    Ok(u64::from_str_radix(s.trim_start_matches("0x"), 16)?)
}

// The R5 cores described in platform_info.toml, by id. Empty for a platform
// file without `[rcpu.<id>]` sections: then an R5 cell gets the first free core
// and the RAM of the pool, as before these sections existed.
pub fn platform_rcpus(platform: &Value) -> Result<Vec<RcpuInfo>, Box<dyn Error>> {
    let mut rcpus = Vec::new();
    if let Some(table) = platform.get("rcpu").and_then(|t| t.as_table()) {
        for (id, section) in table {
            rcpus.push(RcpuInfo {
                id: id.parse()?,
                tcma: hex_field(section, "tcma", id)?,
                tcmb: hex_field(section, "tcmb", id)?,
                tcm_size: hex_field(section, "tcm_size", id)?,
                ddr_start: hex_field(section, "ddr_start", id)?,
                ddr_size: hex_field(section, "ddr_size", id)?,
            });
        }
    }
    rcpus.sort_by_key(|r| r.id);
    Ok(rcpus)
}

// Addresses below this are the R5's own view of its TCMs (ATCM at 0x0, BTCM at
// 0x20000), the same for every core.
const R5_LOCAL_TCM_END: u64 = 0x40000;
const PT_LOAD: u32 = 1;

// The loadable segments (physical address, size in memory) of a 32-bit
// little-endian ELF, the format of the R5 programs.
fn elf32_load_segments(path: &Path) -> Result<Vec<(u64, u64)>, Box<dyn Error>> {
    let data = fs::read(path).map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
    let not_elf = || format!("{} is not a 32-bit little-endian ELF", path.display());
    if data.len() < 52 || &data[0..4] != b"\x7fELF" || data[4] != 1 || data[5] != 1 {
        return Err(not_elf().into());
    }
    let u16_at = |off: usize| data.get(off..off + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
    let u32_at = |off: usize| data.get(off..off + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));

    let phoff = u32_at(0x1c).ok_or_else(not_elf)? as usize;
    let phentsize = u16_at(0x2a).ok_or_else(not_elf)?;
    let phnum = u16_at(0x2c).ok_or_else(not_elf)?;

    let mut segments = Vec::new();
    for i in 0..phnum {
        let ph = phoff + i * phentsize;
        let p_type = u32_at(ph).ok_or_else(not_elf)?;
        let p_paddr = u32_at(ph + 12).ok_or_else(not_elf)? as u64;
        let p_memsz = u32_at(ph + 20).ok_or_else(not_elf)? as u64;
        if p_type == PT_LOAD && p_memsz > 0 {
            segments.push((p_paddr, p_memsz));
        }
    }
    Ok(segments)
}

// The R5 core whose memory holds every loadable segment of the ELF (its TCMs or
// its DDR region), or None if the ELF only uses the TCMs, which any core has.
// An ELF with a segment outside every core's memory, or in the memory of two
// cores, cannot run on any of them.
pub fn rcpu_for_elf(path: &Path, rcpus: &[RcpuInfo]) -> Result<Option<i8>, Box<dyn Error>> {
    let mut core: Option<i8> = None;
    for (addr, size) in elf32_load_segments(path)? {
        let end = addr + size;
        if end <= R5_LOCAL_TCM_END {
            continue;
        }
        let inside = |start: u64, len: u64| addr >= start && end <= start + len;
        let owner = rcpus
            .iter()
            .find(|r| inside(r.ddr_start, r.ddr_size) || inside(r.tcma, r.tcm_size) || inside(r.tcmb, r.tcm_size))
            .ok_or_else(|| {
                format!(
                    "{}: the segment at 0x{:x}-0x{:x} is outside the memory of every R5 core",
                    path.display(),
                    addr,
                    end
                )
            })?;
        match core {
            Some(c) if c != owner.id => {
                return Err(format!(
                    "{}: it has segments in the memory of R5 core {} and of R5 core {}",
                    path.display(),
                    c,
                    owner.id
                )
                .into())
            }
            _ => core = Some(owner.id),
        }
    }
    Ok(core)
}

// Give the cell the R5 core `rcpu`, which must be free.
pub fn rpuconf(c: &mut configGenerator::BackendConfig, rcpu: i8) -> Result<(), Box<dyn Error>> {
    let pos = c
        .rcpus
        .iter()
        .position(|&r| r == rcpu)
        .ok_or_else(|| format!("R5 core {} is not free", rcpu))?;
    c.rcpus.remove(pos);
    c.used_rcpus = vec![rcpu];

    // .rcpus is a bitmask of the cell's remote cores
    c.conf
        .push_str(&format!("\n\t.rcpus = {{\n\t\t0x{:x},\n\t}},\n", 1u64 << rcpu));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The KV260's R5 cores (kria_platform_info.toml)
    fn kv260() -> Vec<RcpuInfo> {
        vec![
            RcpuInfo { id: 0, tcma: 0xffe00000, tcmb: 0xffe20000, tcm_size: 0x10000, ddr_start: 0x3ed00000, ddr_size: 0x8000000 },
            RcpuInfo { id: 1, tcma: 0xffe90000, tcmb: 0xffeb0000, tcm_size: 0x10000, ddr_start: 0x3ad00000, ddr_size: 0x4000000 },
        ]
    }

    // A 32-bit little-endian ELF with these (type, paddr, memsz) program headers
    fn elf(name: &str, segments: &[(u32, u32, u32)]) -> std::path::PathBuf {
        let mut d = vec![0u8; 52];
        d[0..6].copy_from_slice(b"\x7fELF\x01\x01");
        d[0x1c..0x20].copy_from_slice(&52u32.to_le_bytes());
        d[0x2a..0x2c].copy_from_slice(&32u16.to_le_bytes());
        d[0x2c..0x2e].copy_from_slice(&(segments.len() as u16).to_le_bytes());
        for &(p_type, paddr, memsz) in segments {
            let mut ph = vec![0u8; 32];
            ph[0..4].copy_from_slice(&p_type.to_le_bytes());
            ph[8..12].copy_from_slice(&paddr.to_le_bytes());
            ph[12..16].copy_from_slice(&paddr.to_le_bytes());
            ph[20..24].copy_from_slice(&memsz.to_le_bytes());
            d.extend(ph);
        }
        let path = std::env::temp_dir().join(format!("runphi-rpu-test-{}-{}", std::process::id(), name));
        fs::write(&path, d).unwrap();
        path
    }

    #[test]
    fn core_from_elf() {
        // rpu0-freertos-demo.elf: vectors in ATCM, the rest in core 0's DDR
        let p = elf("core0", &[(0x70000001, 0x3ed11f08, 8), (1, 0, 0x730), (1, 0x3ed00000, 0x27b10)]);
        assert_eq!(rcpu_for_elf(&p, &kv260()).unwrap(), Some(0));
        // rpu1-latency-demo.elf: core 1's DDR
        let p = elf("core1", &[(1, 0, 0x5b0), (1, 0x20000, 0x180), (1, 0x3ad00000, 0x277b0)]);
        assert_eq!(rcpu_for_elf(&p, &kv260()).unwrap(), Some(1));
        // only the TCMs: any core
        let p = elf("tcm", &[(1, 0, 0x730), (1, 0x20000, 0x1000)]);
        assert_eq!(rcpu_for_elf(&p, &kv260()).unwrap(), None);
        // linked outside the R5 memory, or in both cores' memory
        let p = elf("outside", &[(1, 0x50000000, 0x1000)]);
        assert!(rcpu_for_elf(&p, &kv260()).is_err());
        let p = elf("both", &[(1, 0x3ed00000, 0x1000), (1, 0x3ad00000, 0x1000)]);
        assert!(rcpu_for_elf(&p, &kv260()).is_err());
        // not an ELF
        let p = std::env::temp_dir().join(format!("runphi-rpu-test-{}-bin", std::process::id()));
        fs::write(&p, b"gic-demo.bin").unwrap();
        assert!(rcpu_for_elf(&p, &kv260()).is_err());
    }

    #[test]
    fn platform_sections() {
        let platform: Value = r#"
[rcpu.1]
tcma = "0xffe90000"
tcmb = "0xffeb0000"
tcm_size = "0x10000"
ddr_start = "0x3ad00000"
ddr_size = "0x4000000"
[rcpu.0]
tcma = "0xffe00000"
tcmb = "0xffe20000"
tcm_size = "0x10000"
ddr_start = "0x3ed00000"
ddr_size = "0x8000000"
"#
        .parse()
        .unwrap();
        let r = platform_rcpus(&platform).unwrap();
        assert_eq!(r.iter().map(|r| r.id).collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!(r[1].ddr_start, 0x3ad00000);
        assert!(platform_rcpus(&"[mem_regions]\nregions = []".parse().unwrap()).unwrap().is_empty());
    }
}
