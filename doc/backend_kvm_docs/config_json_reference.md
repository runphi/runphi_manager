# Container Boot Configuration Reference (`/boot/config.json`)

When runPHI creates a container, it inspects the root filesystem for `/boot/config.json`:
- **Missing `/boot/config.json`**: runPHI delegates execution directly to standard `runc` (`/usr/local/sbin/runc_vanilla`), running the workload as a regular Linux container.
- **Present `/boot/config.json`**: runPHI routes the container to `backend_kvm`, parses the VM parameters, generates a Libvirt domain XML, provisions required storage, and launches QEMU.

All paths in `/boot/config.json` (`inmate`, `ramdisk`, `dtb`, `disk_image`) are relative to the container root filesystem.

---

## Configuration Fields

| Field | Type | Default | Description |
|---|---|---|---|
| `os_var` | string | `""` | OS flavor. Use `"linux"` for Linux guests (sets up kernel command line, console serial, initramfs/disk arguments). Set to `"zephyr"` or other strings for bare-metal, RTOS, or unikernel binaries. |
| `inmate` | string | `"/boot/inmate"` | Path inside the rootfs to the guest kernel or executable binary (e.g. `/boot/Image`, `/boot/vmlinuz`, `/boot/zephyr.elf`). |
| `ramdisk` | string | `""` | Path inside the rootfs to the initial ramdisk (e.g. `/boot/rootfs.cpio.gz`). Used when booting Linux with an initramfs root. |
| `dtb` | string | `""` | Path inside the rootfs to an ARM Device Tree Blob (e.g. `/boot/virt.dtb`). Used on `aarch64` when custom device trees are required. |
| `memory` | integer | `0` (dynamic) | Guest RAM size in MB. When set to `0` or omitted, runPHI checks the OCI container memory limit; if unset, it falls back to 1024 MB for Linux and 32 MB for bare-metal. The host cgroup memory ceiling is set to match. |
| `vcpus` | integer | `0` (dynamic) | Number of virtual CPUs. Evaluated in order: explicit `vcpus` &rarr; `vcpu_pinning.len()` &rarr; OCI CPU quota $\lceil\text{quota}/\text{period}\rceil$ &rarr; default `1`. runPHI warns if `vcpus` exceeds the OCI CPU quota. |
| `vcpu_pinning` | array | `[]` | Static vCPU-to-pCPU affinity map: `[{"vcpu": 0, "pcpu": 2}, {"vcpu": 1, "pcpu": 3}]`. Converted into Libvirt `<cputune><vcpupin/></cputune>` elements. |
| `isolcpu` | string | `""` | Host CPU range isolated from the kernel scheduler (e.g. `"2,3"` or `"2-5"`). Used by IRQ steering to validate protected real-time cores. |
| `nohz_full` | string | `""` | Host CPU range running adaptive tickless mode. Merged with `isolcpu` for safety verification during IRQ affinity configuration. |
| `steer_irq`<br/>*(alias: `irq_steering`)* | array | `null` | Host CPU list where movable host hardware interrupts are redirected while the VM runs (e.g. `[0, 1]`). Original affinities are restored on container deletion. |
| `net` | string / boolean | `"no"` | Networking mode. Supports `true`/`false`, `"user"` / `"slirp"` (QEMU user-mode networking), `"bridge"` (default bridge `virbr0`), `"bridge:<name>"` (explicit host bridge), and `"network:<name>"` (Libvirt virtual network). |
| `netconf` | string | `""` | Additional network parameter. When `net` is `"bridge"`, `netconf` specifies the target host bridge interface name if not encoded in `net`. |
| `disk_type` | string | `""` | Root disk backend: `""` (initramfs in RAM), `"file"` (raw image), or `"lvm"` (host logical volume). With `"lvm"`, runPHI provisions an ext4 logical volume (`/dev/<vg>/lv_<id>`), populates it with the rootfs, and sets `root=/dev/vda rw`. |
| `disk_image` | string | `""` | Path inside the rootfs to a raw disk image when `disk_type` is `"file"`. |
| `disk_size` | integer | `0` | Disk size in MB when `disk_type` is `"lvm"`. If `0`, runPHI auto-calculates rootfs size + 30% headroom + 64 MB minimum metadata allowance. |

---

## Configuration Examples

### Bare-Metal / RTOS (Zephyr)
Boots a raw ELF binary directly without initramfs or Linux command line:

```json
{
  "os_var": "zephyr",
  "inmate": "/boot/zephyr.elf",
  "memory": 64,
  "vcpus": 1,
  "net": "no"
}
```

### Linux with Initramfs
Boots a Linux kernel and loads the root filesystem entirely into RAM:

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

### Linux with Image File Disk
Mounts an ext4 disk image packaged inside the container as `/dev/vda`:

```json
{
  "os_var": "linux",
  "inmate": "/boot/Image",
  "disk_type": "file",
  "disk_image": "/boot/rootfs.img",
  "memory": 512,
  "vcpus": 1,
  "net": "no"
}
```

### Linux with Host LVM Storage
Provisions an ext4 logical volume from the host volume group on container creation and tears it down on deletion:

```json
{
  "os_var": "linux",
  "inmate": "/boot/Image",
  "disk_type": "lvm",
  "disk_size": 2048,
  "memory": 1024,
  "vcpus": 2,
  "net": "user"
}
```

The host must have the target Volume Group available (`test-vg` by default, or configured via `/usr/share/runPHI/kvm_lvm_vg`).

### Real-Time Guest with Core Pinning and IRQ Steering
Dedicates physical host cores to guest vCPUs and steers host hardware interrupts away from real-time cores:

```json
{
  "os_var": "linux",
  "inmate": "/boot/Image",
  "ramdisk": "/boot/rootfs.cpio.gz",
  "memory": 2048,
  "vcpus": 2,
  "vcpu_pinning": [
    { "vcpu": 0, "pcpu": 2 },
    { "vcpu": 1, "pcpu": 3 }
  ],
  "isolcpu": "2,3",
  "nohz_full": "2,3",
  "steer_irq": [0, 1],
  "net": "no"
}
```

### Bridged Host Networking
Connects the guest directly to a host Linux bridge (`docker0`, `virbr0`, or custom bridge) using a virtio NIC:

```json
{
  "os_var": "linux",
  "inmate": "/boot/Image",
  "ramdisk": "/boot/rootfs.cpio.gz",
  "memory": 1024,
  "vcpus": 2,
  "net": "bridge:docker0"
}
```

Or using a Libvirt managed network:

```json
{
  "os_var": "linux",
  "inmate": "/boot/Image",
  "ramdisk": "/boot/rootfs.cpio.gz",
  "memory": 1024,
  "vcpus": 2,
  "net": "network:default"
}
```

---

## Combining `/boot/config.json` with Docker Resource Flags

runPHI applies standard Docker resource limits (`docker run` flags) to the hypervisor process via Linux cgroups.

```bash
docker run -d \
  --runtime=runphi \
  --cpuset-cpus=2,3 \
  --memory=2048m \
  --cpu-quota=100000 \
  --cpu-period=100000 \
  my-kvm-image:latest
```

When started with these flags:
1. Docker passes the resource specifications into the container OCI `config.json`.
2. `cgroups.rs` creates the cgroup hierarchy under `/sys/fs/cgroup/runphi/<container_id>/` and configures `cpuset.cpus`, `cpu.max`, and `memory.max`.
3. After Libvirt spawns QEMU, runPHI moves the QEMU main PID into `/sys/fs/cgroup/runphi/<container_id>/cgroup.procs`.
4. All QEMU emulator threads (event loop, virtio I/O workers, timer handlers) are constrained by these limits, complementing any guest-internal vCPU pinning specified in `/boot/config.json`.

---

## Packaging and Running a Container Image

### 1. Prepare Guest Files
Organize the guest kernel, ramdisk, and configuration into a build directory:

```bash
mkdir -p guest_build/boot
cd guest_build

cp /path/to/Image boot/Image
cp /path/to/rootfs.cpio.gz boot/rootfs.cpio.gz
```

Write `boot/config.json`:
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

### 2. Create the Dockerfile
```dockerfile
FROM scratch
COPY boot/ /boot/
CMD ["/bin/sh"]
```

### 3. Build and Run

Build the image:
```bash
docker build -t runphi-linux-guest:latest .
```

Run with runPHI:
```bash
# Basic run
docker run --rm -it --runtime=runphi runphi-linux-guest:latest

# Run with host cgroup constraints
docker run --rm -it \
  --runtime=runphi \
  --cpuset-cpus=2,3 \
  --memory=1024m \
  runphi-linux-guest:latest

# Run with LVM storage (requires host volume group prepared)
docker run --rm -it --runtime=runphi runphi-lvm-guest:latest
```

### Runtime Inspection

Check running Libvirt domains:
```bash
sudo virsh list --all
```

Check runPHI execution logs:
```bash
cat /usr/share/runPHI/log.txt
```

Verify cgroup process assignment:
```bash
# cgroups v2
cat /sys/fs/cgroup/runphi/<container_id>/cgroup.procs
cat /sys/fs/cgroup/runphi/<container_id>/cpuset.cpus
```

Check active LVM volumes:
```bash
sudo lvs
```

