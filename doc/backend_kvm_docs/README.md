# runPHI — KVM Backend (`backend_kvm`)

`backend_kvm` is the runPHI hypervisor backend for running partitioned containers as hardware-virtualized guests using KVM and Libvirt (`virsh`). It allows container engines (Docker, containerd, Kubernetes) to launch virtual machine workloads through standard OCI lifecycle commands.

## Documentation Index

- **[Architecture and Lifecycle](architecture_and_lifecycle.md)**  
  Backend architecture, host prerequisites (including LVM volume group setup), build steps, OCI-to-Libvirt lifecycle dispatch, process supervision (watcher), cgroups v1/v2 confinement, real-time CPU/IRQ isolation, and on-disk state.

- **[Config Generator Deep Dive](config_generator.md)**  
  Internal structure of `crates/backend_kvm/src/configGenerator.rs`, the `BackendConfig` struct, Domain XML assembly, and submodule references (`cpu.rs`, `boot.rs`, `disk.rs`, `network.rs`, `irq.rs`).

- **[Container Configuration Reference (`/boot/config.json`)](config_json_reference.md)**  
  Complete schema for `/boot/config.json`, practical templates (bare-metal, initramfs, file disk, LVM storage, real-time pinning, Docker cgroups constraints), and steps to build and run container images.

## Features

| Feature | Implementation |
|---|---|
| Hypervisor | KVM via Libvirt (`virsh create/resume/suspend/destroy`). Automatic fallback to QEMU TCG if `/dev/kvm` is absent. |
| Architectures | `x86_64` (machine `q35`, APIC/ACPI) and `aarch64` (machine `virt`, GICv3). |
| cgroups Sandboxing | Uses `libcgroups` (v1/v2) to place the QEMU main PID and helper threads under host cgroups, enforcing OCI CPU quotas, `cpuset`, and memory limits. |
| CPU Pinning | Direct vCPU-to-pCPU affinity via Libvirt `<cputune>` and `<vcpupin>`. |
| IRQ Steering | Automatic migration of host IRQs (`/proc/irq/*/smp_affinity_list`) away from isolated cores to housekeeper cores; restored on teardown. |
| Host Storage | File-backed raw disk images or dynamically provisioned host LVM logical volumes formatted with ext4. |
| Process Tracking | Supervisor watcher process bridging Libvirt child processes to containerd PID tracking. |
| Networking | User-mode NAT (SLIRP), host Linux bridges (`docker0`, `virbr0`), and Libvirt managed networks. |

## Quick Start

A runPHI partitioned container image provides a boot payload and a `/boot/config.json` inside its root filesystem:

```json
{
  "os_var": "linux",
  "inmate": "/boot/Image",
  "ramdisk": "/boot/rootfs.cpio.gz",
  "memory": 1024,
  "vcpus": 2,
  "net": "user"
}
```

Running the container:

```bash
docker run --rm -it --runtime=runphi <image_name>
```

When invoked, runPHI detects `/boot/config.json`, generates a Libvirt `domain.xml` in `/run/runPHI/<id>/`, starts the domain in a paused state (`virsh create --paused`), attaches the QEMU process to a dedicated cgroup, starts a supervisor watcher for containerd, and unpauses the guest on OCI `start`.
