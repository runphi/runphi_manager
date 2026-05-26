## To install rust 
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
## install crosscompiler
apt install gcc-aarch64-linux-gnu 
## To build
Pick the hypervisor backend at build time via a Cargo feature (`jailhouse` is the default):

    # Jailhouse (default)
    cargo build --release --target=aarch64-unknown-linux-gnu

    # Xen
    cargo build --release --target=aarch64-unknown-linux-gnu -p runphi \
        --no-default-features --features xen

Or use the wrapper script from the repo root:

    ./compile_rust.sh             # jailhouse
    ./compile_rust.sh xen

## Adding a new hypervisor backend

Backends are crates that expose a uniform API. The `runphi` crate picks exactly one at build time via a Cargo feature. The existing `backend_jailhouse` and `backend_xen` are reference implementations.

To add a backend called `<name>` (replace `<name>` everywhere):

**1. Create the crate at `crates/backend_<name>/`** with this layout:

    crates/backend_<name>/
        Cargo.toml
        src/
            lib.rs
            timer.rs
            configGenerator.rs   # plus submodules as needed

`Cargo.toml`:

```toml
[package]
name = "backend_<name>"
version = "0.1.0"
edition = "2021"

[dependencies]
f2b     = { path = "../frontend_to_backend" }
logging = { path = "../logging" }
# Plus whatever the platform needs (libc, nix, serde_json, ...).
```

The workspace at `rust_runphi/Cargo.toml` uses `members = ["crates/*"]`, so the new crate is picked up automatically.

**2. Implement the backend API in `src/lib.rs`.** The runphi frontend calls these seven items by name, with these exact signatures. See `crates/backend_jailhouse/src/lib.rs` for a worked example.

```rust
use std::error::Error;
use f2b;

#[allow(non_snake_case)]
pub mod configGenerator;   // must expose: fn config_generate(fc: &f2b::FrontendConfig) -> ...
pub mod timer;

pub fn startguest   (containerid: &str, crundir: &str) -> Result<(), Box<dyn Error>> { /* ... */ }
pub fn stopguest    (containerid: &str, crundir: &str) -> Result<(), Box<dyn Error>> { /* ... */ }
pub fn destroyguest (containerid: &str, crundir: &str) -> Result<(), Box<dyn Error>> { /* ... */ }
pub fn cleanup      (containerid: &str, crundir: &str) -> Result<(), Box<dyn Error>> { /* ... */ }
pub fn createguest  (fc: &f2b::FrontendConfig, ic: &f2b::ImageConfig) -> Result<(), Box<dyn Error>> { /* ... */ }
pub fn storeinfo    (fc: &f2b::FrontendConfig, ic: &f2b::ImageConfig) -> Result<(), Box<dyn Error>> { /* ... */ }
```

**3. Implement the tick source in `src/timer.rs`.** The platform-specific counter reader lives here; `logging::timer` consumes it through a trait so the rest of the codebase stays hypervisor-independent.

```rust
use logging::timer::TickSource;

pub struct MyTickSource { /* whatever state the platform needs */ }

impl MyTickSource {
    pub fn new() -> std::io::Result<Self> { /* open device / map memory / ... */ }
}

impl TickSource for MyTickSource {
    fn read_ticks(&self) -> u64 { /* return a monotonic 64-bit tick count */ }
}

// Called once from main().
pub fn install() -> std::io::Result<()> {
    logging::timer::initialize_with(Box::new(MyTickSource::new()?))
}
```

Reference: `crates/backend_jailhouse/src/timer.rs` (MMIO mmap via `/dev/mem`) and `crates/backend_xen/src/timer.rs` (char-device read of `/dev/arm_timer`).

**4. Register the backend in `crates/runphi/Cargo.toml`** by adding one optional dependency and one feature:

```toml
[features]
default = ["jailhouse"]
jailhouse = ["dep:backend_jailhouse"]
xen       = ["dep:backend_xen"]
<name>    = ["dep:backend_<name>"]        # new

[dependencies]
backend_jailhouse = { path = "../backend_jailhouse", optional = true }
backend_xen       = { path = "../backend_xen",       optional = true }
backend_<name>    = { path = "../backend_<name>",    optional = true }   # new
```

**5. Wire the alias and version string in `crates/runphi/src/main.rs`** by adding three lines and extending the existing guards:

```rust
#[cfg(feature = "<name>")]
pub use backend_<name> as backend;

#[cfg(feature = "<name>")]
const VERSION_STR: &str = "0.5.7 (backend: <name>)";

// Extend the "no backend" guard to mention the new option:
#[cfg(not(any(feature = "jailhouse", feature = "xen", feature = "<name>")))]
compile_error!("Select a backend: --features jailhouse | xen | <name>");

// Add pairwise mutual-exclusion guards involving the new backend:
#[cfg(all(feature = "jailhouse", feature = "<name>"))]
compile_error!("Backends jailhouse and <name> are mutually exclusive");
#[cfg(all(feature = "xen", feature = "<name>"))]
compile_error!("Backends xen and <name> are mutually exclusive");
```

No other file in `runphi` needs to change. Calls like `backend::startguest(...)` and `backend::timer::install()` resolve through the cfg-gated `pub use` at the crate root.

**6. Extend the build script** at `rust_runphi/compile_rust.sh` so the wrapper accepts the new backend name. Add `<name>` to the case-statement allowlist alongside `jailhouse` and `xen`.

**7. (Optional) Update the docs.** Mention the new backend in the architecture section of the top-level `README.md`.

**Sanity check.** After these changes, all three of these should succeed:

    cargo check --no-default-features --features jailhouse
    cargo check --no-default-features --features xen
    cargo check --no-default-features --features <name>

And these should fail with the expected `compile_error!` messages:

    cargo check --no-default-features                                  # neither feature
    cargo check --no-default-features --features jailhouse,<name>      # both features

## Adding support for a new target board
To add support for a new board in the config generator you need to know how a standard jailhouse configuration for that board is done. In the module templates.rs are available various templates for the preamble, the memory regions, the interrupt request chip and the pci devices. 
Here is an example for a memory region mapping an UART device.
```rust
pub const UART_TEMPLATE: &'static str = r#"
/* UART */ {
    .phys_start = {phys_start},
    .virt_start = {virt_start},
    .size = {size},
    .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
        JAILHOUSE_MEM_IO | JAILHOUSE_MEM_ROOTSHARED,
},
"#;
```
Note that the physical start, virtual start and size fields can be customized by using board specific values.
If none of the templates can be used for your board standard configuration you can add others. 
The configuration is then generated by giving runphi a .toml file containing the templates to use for your target and the board specific values with which to fill the templates.
Here is an examble for the KriaKV260 board:
```toml
[mem_regions]
regions = ["IVSHMEM_TEMPLATE", "UART_TEMPLATE", "TCMA_TEMPLATE", "TCMB_TEMPLATE", "RAM_TEMPLATE", "COMM_REGION_TEMPLATE"]

[jailhouse_preamble]
preamble = "ULTRASCALE_PREAMBLE"

[IVSHMEM_TEMPLATE]
address = "0x060000000"

[UART_TEMPLATE]
phys_start = "0xff010000"
virt_start = "0xff010000"
size = "0x1000"

[TCMA_TEMPLATE]
phys_start = "0xffe00000"
virt_start = "0xffe00000"
size = "0x00010000"

[TCMB_TEMPLATE]
phys_start = "0xffe20000"
virt_start = "0xffe20000"
size = "0x00010000"

[RAM_TEMPLATE]
phys_start = "0x3ed00000"
virt_start = "0"
size = "0x8000000"

[COMM_REGION_TEMPLATE]
# No additional parameters

[devices]
devs = ["IRQ_CHIP_BOARD_TEMPLATE", "PCI_DEVICE_TEMPLATE"]

[IRQ_CHIP_BOARD_TEMPLATE]
gic_address = "0xf9010000"
uart_pin = "33"
ivshmem_pin = "146"

[PCI_DEVICE_TEMPLATE]
ivshmem_bdf = "1"
```

Moreover, it is also necessary to gice the board an initial state file with .toml extension. 
Here's an example for the same target:
```toml
[containerid]
ids = []

[available_memory]
memory = "0x70000000, 0x7f800000"

[free_segments]
segments= ["0x70000000, 0x7f800000"]

[free_pci_devices_bdf]
bdf = [1,2]

[free_rcpus]
ids = [0,1]
```
This file must be populated with the resources that we can assign to the partitioned container defined in the particular root cell configuration used. Most important is the memory available to be assigned to the non-root cells which will also be the initial free memory segment. This value changes depending from the board and must be known in order not to cause crashes.
