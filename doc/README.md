# runPHI — Architecture and internals

A deeper-dive companion to the top-level [README](../README.md). Read that first for what runPHI is, how to build it, and the user-facing workflow. This document covers the internals: what each crate does, how the OCI lifecycle is dispatched, how the forwarding-to-runc decision is made, and where the on-disk state lives.

## Architecture

```mermaid
flowchart TB
    %% Upper orchestration layer
    subgraph upper["Container orchestration layer"]
        direction LR
        kubelet
        dockerd
        containerd
        CRIO["CRI-O"]
    end

    %% runphi binary
    subgraph runphi_bin["runphi binary (one backend baked in at build time)"]
        direction TB
        
        subgraph runphi_crate["runphi crate (hypervisor-independent)"]
            main["main: OCI dispatch"]
            forwarding["forwarding decision<br/>(annotation / /boot/config.json /<br/>/run/runPHI/&lt;id&gt;/)"]
        end
        
        oci["liboci_cli<br/>(OCI command parsing)"]
        f2b["frontend_to_backend<br/>(FrontendConfig, ImageConfig)"]
        logging["logging<br/>(log file + TickSource trait)"]
        
        subgraph active_backend["backend_jailhouse OR backend_xen (Cargo feature selects one at build time)"]
            direction TB
            backend_api["startguest / stopguest /<br/>createguest / destroyguest /<br/>cleanup / storeinfo"]
            configGenerator["configGenerator<br/>(builds cell/domU config)"]
            tick_source["TickSource impl<br/>(/dev/mem MMIO  |  /dev/arm_timer)"]
        end
    end

    %% Below runphi
    runc["runc_vanilla<br/>(/usr/local/sbin/runc_vanilla)"]
    hypervisor["Jailhouse cell  |  Xen domU"]

    %% Connections
    kubelet --> containerd
    kubelet --> CRIO
    dockerd --> containerd
    containerd -- "exec runc" --> main
    CRIO --> main

    main --> oci
    main --> forwarding
    main --> backend_api
    main --> logging
    backend_api --> f2b
    backend_api --> configGenerator
    logging -- "installs at startup" --> tick_source

    forwarding -- "standard containers<br/>(no /boot/config.json)" --> runc
    backend_api -- "manages" --> hypervisor
```

Walking the diagram:

- **One binary, one backend.** `runphi` is a single binary; which hypervisor backend is compiled in is decided by Cargo features (`jailhouse` default, or `--features xen`). `runphi --version` prints which one. There is no runtime backend switch.
- **Hypervisor-independent core.** Everything in the `runphi`, `logging`, `liboci_cli`, and `frontend_to_backend` crates is the same regardless of backend. The active backend is re-exported at the `runphi` crate root as `backend`, so the rest of the codebase calls `backend::createguest(...)` without knowing which one it is.
- **Forwarding fork.** Before runPHI takes ownership of a container, `forwarding` decides whether the OCI call should be handled here or forwarded to vanilla runc. Decision rules in order: (1) explicit OCI annotation `org.runphi.runtime`, (2) at `create`, presence of `/boot/config.json` in the rootfs, (3) for later commands, presence of `/run/runPHI/<id>/`.
- **TickSource boundary.** The `logging::timer` module is hypervisor-independent, but its tick source is not. Each backend implements a tiny `TickSource` trait (one method, `read_ticks() -> u64`) and the backend's `timer::install()` is called once from `main`. After that, `timer::capture()` works the same way everywhere.

## Crate layout

| Crate | Hypervisor-aware? | Purpose |
|---|---|---|
| `runphi` | no | Binary. Parses OCI args, decides forwarding, dispatches to the backend, owns `main()`. |
| `liboci_cli` | no | OCI command-line parsing. Subset of the OCI runtime spec. |
| `frontend_to_backend` | no | Shared data structures (`FrontendConfig`, `ImageConfig`) used as the API between `runphi` and any backend. Parses `/boot/config.json` from the container rootfs. |
| `logging` | no | Log file writer; `timer` module with the `TickSource` trait and `u64` tick API. |
| `backend_jailhouse` | yes | Jailhouse backend: drives `jailhouse cell {create,load,start}`, MMIO mmap of `/dev/mem` for the timer. |
| `backend_xen` | yes | Xen backend: drives `xl create/unpause/destroy`, reads `/dev/arm_timer` for the timer. |

The two backend crates expose the same seven items (`startguest`, `stopguest`, `destroyguest`, `cleanup`, `createguest`, `storeinfo`, plus the `configGenerator` module). Adding a new backend means producing the same surface. See `rust_runphi/README.md` § *Adding a new hypervisor backend* for the step-by-step recipe.

## Lifecycle of a partitioned container

containerd (or CRI-O, or any OCI-compliant runtime client) execs `/usr/bin/runc` for every container lifecycle event. Because the `switch_to_runphi.sh` script replaced the distro's `runc` with runPHI, containerd is actually invoking us. The dispatch in `runphi/src/main.rs` looks like this for each OCI command:

```
create  → read bundle config.json → forwarding::decide_create
         → (forward to runc_vanilla and exit)  OR
           (mkdir /run/runPHI/<id>/ → frontend::commands::create
            → backend::createguest → backend::storeinfo)

start   → forwarding::decide_existing (checks /run/runPHI/<id>/)
         → forward  OR  frontend::commands::start → backend::startguest

kill    → same forwarding check  →  backend::stopguest + backend::destroyguest
delete  → same forwarding check  →  backend::stopguest + backend::destroyguest + backend::cleanup
state   → same forwarding check  →  read bundle/pidfile/rootfs from /run/runPHI/<id>/
```

`createguest` is the heavy step. On Jailhouse it calls `jailhouse cell create` with the cell config produced by `configGenerator`, then `jailhouse cell load` for the inmate binary. On Xen it builds a domU config and calls `xl create -p` (paused), with `startguest` later running `xl unpause`.

## On-disk state

| Path | Created by | Contents |
|---|---|---|
| `/usr/share/runPHI/log.txt` | `logging::init_logger` | All runPHI log lines and (optionally) timer phase markers. |
| `/run/runPHI/<id>/` | `runphi/src/main.rs` on `create` | Per-container working dir. Its presence is the source of truth for "this is a runPHI-managed container" in subsequent OCI commands. |
| `/run/runPHI/<id>/bundle` | `backend::storeinfo` | Absolute path to the OCI bundle. |
| `/run/runPHI/<id>/pidfile` | `backend::storeinfo` | Path to the container's pidfile (the init process). |
| `/run/runPHI/<id>/rootfs` | `backend::storeinfo` | Absolute path to the mounted rootfs. |
| `/run/runPHI/<id>/config<id>.conf` | backend `configGenerator` | Hypervisor-specific config: the Jailhouse `.c` cell file or the Xen domU config, depending on backend. |
| `<rootfs>/boot/config.json` | container image (input, not produced by us) | Boot parameters for the partitioned cell: which kernel/inmate to load, OS variant, devices, memory layout, etc. Parsed by `frontend_to_backend::ImageConfig::get_from_file`. |
| `/usr/local/sbin/runc_vanilla` | `switch_to_runphi.sh` | Backup of the distro's original `runc`, executed when runPHI forwards a non-runPHI container. |

## Pointers into the code

- OCI dispatch: [rust_runphi/crates/runphi/src/main.rs](../rust_runphi/crates/runphi/src/main.rs)
- Forwarding decision: [rust_runphi/crates/runphi/src/forwarding.rs](../rust_runphi/crates/runphi/src/forwarding.rs)
- Frontend command handlers: [rust_runphi/crates/runphi/src/frontend/commands.rs](../rust_runphi/crates/runphi/src/frontend/commands.rs)
- Shared backend API types: [rust_runphi/crates/frontend_to_backend/src/lib.rs](../rust_runphi/crates/frontend_to_backend/src/lib.rs)
- Timer trait: [rust_runphi/crates/logging/src/timer.rs](../rust_runphi/crates/logging/src/timer.rs)
- Jailhouse backend: [rust_runphi/crates/backend_jailhouse/src/lib.rs](../rust_runphi/crates/backend_jailhouse/src/lib.rs)
- Xen backend: [rust_runphi/crates/backend_xen/src/lib.rs](../rust_runphi/crates/backend_xen/src/lib.rs)
