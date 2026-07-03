# How to run a Linux Xen domU with runPHI on x86

This guide walks through running a Linux container as a Xen **PVH domU** with
runPHI on an x86_64 Xen Dom0: building runPHI for x86, installing it as a Docker
runtime, producing a PVH-capable kernel + initramfs, packaging them into a
container image, and launching it. It also covers booting from a virtual disk
instead of an initramfs.

A note on the build: runPHI's usual target is **aarch64**, cross-compiled (via
`compile_rust.sh` / the `runphi_builder` Docker image with
`gcc-aarch64-linux-gnu`). On x86 you build **natively** for the host — there is
no cross-compilation.

---

## 0. Host prerequisites (Xen Dom0, x86_64)

- Xen is running and Dom0 is Linux x86_64: `sudo xl info` works and
  `sudo xl list` shows `Domain-0`.
- Xen toolstack present: `xl`. Also `lscpu` (runPHI reads CPU topology) —
  standard on any Linux.
- Docker (or containerd) installed and running.
- A **Xen-PVH-capable** x86_64 kernel `Image` and an **initramfs**
  `rootfs.cpio.gz` containing an `/init`. Section 3 builds both; the kernel
  config requirements live there.
- runPHI runs as **root** (dockerd/containerd invoke it as root); it creates
  `/usr/share/runPHI` and `/run/runPHI` and calls `xl`.

---

## 1. Build runphi for x86 (native, not the aarch64 cross flow)

Build on the Dom0 (or any x86_64 Linux with the same/older glibc — building
directly on the Dom0 avoids glibc-mismatch surprises):

```sh
# Rust toolchain, if not already present
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

cd rust_runphi
# Native x86_64 build, Xen backend. The workspace default feature is
# "jailhouse", so you MUST pass --no-default-features --features xen and build
# the runphi binary crate with -p runphi. No --target: the host IS x86_64.
cargo build --release -p runphi --no-default-features --features xen
```

Result: `rust_runphi/target/release/runphi` — `file …/runphi` reports
`ELF 64-bit … x86-64`.

> The aarch64-only `compile_rust.sh` wrapper is **not** used here; it hardcodes
> `--target aarch64-unknown-linux-gnu`.

**Optional, fully static binary** (build on a newer box, run on an older Dom0):

```sh
rustup target add x86_64-unknown-linux-musl
cargo build --release -p runphi --no-default-features --features xen \
    --target x86_64-unknown-linux-musl
# -> target/x86_64-unknown-linux-musl/release/runphi  (no glibc dependency)
```

### x86-specific behavior

- **Timer**: the Xen tick source reads `/dev/arm_timer`, which is ARM-only. A
  missing timer is **non-fatal** on x86 (a warning is logged and runphi
  continues); boot-time instrumentation is simply disabled.
- **`gic_version`**: an ARM GIC concept that `xl` rejects on x86 — runPHI emits
  it only on aarch64 builds, so x86 configs omit it.
- **PVH**: a Linux guest gets `type = "pvh"` on x86 (omitted on ARM).

---

## 2. Install runphi on the Dom0

Two options. **Option A (dedicated Docker runtime)** is the least invasive: it
does **not** replace the system `runc` — only containers launched with
`--runtime=runphi` go through runPHI.

### Common one-time setup (both options)

```sh
sudo mkdir -p /usr/share/runPHI /usr/local/sbin
# caronte: the keep-alive helper runphi spawns so containerd sees a live init.
# It is a plain bash script (arch-independent).
sudo cp target/runPHI_cell_configs/caronte /usr/share/runPHI/caronte
sudo chmod +x /usr/share/runPHI/caronte
# runc_vanilla: runphi execs this when it forwards a non-runPHI container.
sudo cp -n "$(command -v runc)" /usr/local/sbin/runc_vanilla
```

### Option A — register runphi as a separate Docker runtime

```sh
sudo install -m0755 rust_runphi/target/release/runphi /usr/local/sbin/runphi
```

Add to `/etc/docker/daemon.json` (merge with any existing content):

```json
{
  "runtimes": {
    "runphi": { "path": "/usr/local/sbin/runphi" }
  }
}
```

```sh
sudo systemctl restart docker     # or: sudo service docker restart
```

Containers without `--runtime=runphi` keep using the stock runtime untouched.
Confirm the runtime registered with `sudo docker info --format '{{.Runtimes}}'`
(it should list `runphi`).

### Option B — replace the system runc (whole-node, like the ARM boards)

```sh
sudo cp rust_runphi/target/release/runphi /usr/bin/runc
```

Now **every** container is dispatched through runPHI; it auto-forwards anything
without `/boot/config.json` to `/usr/local/sbin/runc_vanilla`. Revert with
`sudo cp /usr/local/sbin/runc_vanilla /usr/bin/runc`.

---

## 3. Build the x86 PVH kernel + initramfs (Buildroot)

You need two artifacts to ship in the container: a Xen-PVH-capable kernel
`Image` (bzImage) and an initramfs `rootfs.cpio.gz` with an `/init`. Buildroot
produces both in one build.

```sh
git clone --depth=1 https://git.buildroot.net/buildroot
cd buildroot
make qemu_x86_64_defconfig
make menuconfig        # top-level options (initramfs format)
make linux-menuconfig  # kernel options below
make -j"$(nproc)"
```

In **`make linux-menuconfig`**, enable (all built-in, `=y`):

- **`CONFIG_XEN`, `CONFIG_XEN_PVH`** — Xen PVH guest init. `CONFIG_PVH` alone is
  **not** enough: without `CONFIG_XEN_PVH` the guest triple-faults at boot with
  `Missing xen PVH initialization`. (`XEN_PVH` depends on `XEN`, `XEN_PVHVM`,
  `ACPI`.)
- **`CONFIG_HVC_XEN`, `CONFIG_HVC_XEN_FRONTEND`** — the `hvc0` console; without
  them `console=hvc0` produces no output and the guest looks dead even when it
  boots fine.
- **`CONFIG_XEN_BLKDEV_FRONTEND`** — for the disk modes in Section 6
  (`/dev/xvda`); harmless to include always.

In the top-level **`make menuconfig`**, set *Filesystem images → cpio the root
filesystem → Compression = gzip* to produce `rootfs.cpio.gz`, and make sure a
kernel is enabled under *Kernel*.

> **host-qemu build failure.** `qemu_x86_64_defconfig` also builds a host QEMU,
> which can fail to compile against a Dom0's system Xen headers
> (`error: static declaration of 'xendevicemodel_set_irq_level' …`). You don't
> need QEMU to run under Xen — disable it (*Host utilities → host qemu*, or set
> `# BR2_PACKAGE_HOST_QEMU is not set` and `make olddefconfig`) and rebuild.

Copy the outputs out of `output/images/`:

```sh
cp output/images/bzImage        <container-dir>/Image
cp output/images/rootfs.cpio.gz <container-dir>/rootfs.cpio.gz
```

---

## 4. Package the kernel + initramfs into a container image

runPHI reads the guest's boot parameters from `/boot/config.json` inside the
container rootfs and boots the kernel it points at. Use
`target/demo_containers/linux/` as a template.

`target/demo_containers/linux/config.json`:

```json
{ "os_var": "linux", "inmate": "/boot/Image", "ramdisk": "/boot/rootfs.cpio.gz", "memory": 512, "net": "no" }
```

The `Dockerfile` ships the kernel, initramfs, and boot config under `/boot`:

```sh
cd target/demo_containers/linux
# copy your x86_64 Image and rootfs.cpio.gz here first
# (the committed demo artifacts are ARM64 — replace them)
docker build -t runphi-linux-demo .
```

The container only **carries** `/boot/{Image,rootfs.cpio.gz,config.json}`; its
`CMD` is irrelevant (caronte keeps the container object alive while the domU
runs). runPHI dispatches any image that has `/boot/config.json` through the Xen
backend; images without it are forwarded to the stock runtime. An image whose
`config.json` sets `os_var` to something other than `"linux"` (e.g. `"zephyr"`)
boots as a **bare-metal inmate** instead — kernel only, no ramdisk or disk.

---

## 5. Run the Linux container

```sh
# Option A (dedicated runtime):
docker run -d --runtime=runphi --name linuxdom runphi-linux-demo
# Option B (system runc replaced):
# docker run -d --name linuxdom runphi-linux-demo
```

The domU appears in `xl list`, named after the container id (truncated to 24
chars). Attach to its console to watch Linux boot:

```sh
sudo xl list
sudo xl console <domain-name>   # kernel log, initramfs unpack, /init; detach with Ctrl-]
```

runPHI writes the generated domain config and per-container state under
`/run/runPHI/<id>/`:

```sh
cat /run/runPHI/<id>/config.cfg
```

A Linux guest config looks like:

```
name = "<id>"
kernel = "/var/lib/docker/.../boot/Image"
type = "pvh"
ramdisk = "/var/lib/docker/.../boot/rootfs.cpio.gz"
extra = "console=hvc0"
vcpus = N
cpus = [...]
memory = 512
maxmem = 512
on_crash="preserve"
```

Guest memory comes from `"memory"` (MB) in `/boot/config.json`; if omitted it
falls back to docker's `--memory` limit (converted from bytes), then a 1024 MB
default. If the domain fails to build for lack of RAM, either lower `"memory"`
or free some on the Dom0 (`sudo xl mem-set Domain-0 <MB>`).

Tear down:

```sh
docker rm -f linuxdom     # stops the domU and removes /run/runPHI/<id>
```

---

## 6. Disk-backed root modes (file / LVM)

Besides the initramfs default, a Linux guest can boot from a disk attached as
`xvda` (runPHI adds `root=/dev/xvda` to `extra`). The mode is selected by
`disk_type` in `/boot/config.json`. For these the kernel needs
`CONFIG_XEN_BLKDEV_FRONTEND=y` built in (Section 3).

### 6a. File-backed disk (used in place)

The container ships a raw ext4 image; runPHI attaches it directly from the
mounted container rootfs — nothing is provisioned or cleaned on the host.

```sh
# create an ext4 image holding a rootfs (busybox or a debootstrap tree)
truncate -s 512M rootfs.img
mkfs.ext4 -F rootfs.img
sudo mount -o loop rootfs.img /mnt && sudo cp -a <your-rootfs>/. /mnt/ && sudo umount /mnt
```

Use `config.file-disk.json` as `/boot/config.json` (it sets
`disk_image: "/boot/rootfs.img"`), place `rootfs.img` next to `Image` in the
Dockerfile, then build and run as in Sections 4–5. Result:

- `config.cfg` gets `disk = ['<...>/boot/rootfs.img,raw,xvda,rw']` and
  `extra = "console=hvc0 root=/dev/xvda"`;
- Linux mounts `/dev/xvda` as root;
- guest writes live in the container's writable layer: they persist across a
  guest reboot and are discarded on `docker rm -f`. Nothing is left on the host.

### 6b. LVM-backed disk (clone of the container rootfs)

At `create`, runPHI provisions `lv_<id>` in a volume group, formats it ext4, and
copies the container rootfs into it — the docker image becomes the VM's
persistent root. At teardown the LV is removed.

Host setup:

```sh
# a volume group with free space (default expected name: test-vg)
sudo vgs
# override the VG name if yours differs:
echo myvg | sudo tee /usr/share/runPHI/xen_lvm_vg
# mkfs.ext4 (e2fsprogs) must be installed
```

Use `config.lvm.json` as `/boot/config.json` (optional `disk_size` in MB;
default = rootfs size +30% +64 MB). The container rootfs must be a bootable
userland (init at `/sbin/init` or similar). Behavior:

- `create` is slower (lvcreate + mkfs + copy);
- `sudo lvs` shows `lv_<container-id>` while the container exists;
- `/run/runPHI/<id>/disk` records `<lv_path> <size_mb>`;
- `config.cfg` has `disk = ['/dev/<vg>/lv_<id>,raw,xvda,rw']`;
- guest writes persist for the container's lifetime; `docker rm -f` removes the
  domain **and** the LV.

---

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `runphi`/`runc` exits immediately, nothing in log | `/usr/share/runPHI` not writable — must run as root. The logger auto-creates the dir; ensure the process is root. |
| Guest panics: `No init found` / `Kernel panic - not syncing: VFS: Unable to mount root` | The `rootfs.cpio.gz` is not a valid initramfs (missing `/init`) — it's a disk-style rootfs. Rebuild it as an initramfs, or switch to a file-backed disk mode. |
| `xl create` fails: `unknown config option 'gic_version'` | An ARM build was installed on x86. Rebuild natively for x86_64 (Section 1). |
| `runphi --version` (or every run) fails with `NotFound … /dev/arm_timer` | An old runphi build with a fatal timer install. Rebuild from current source (Section 1); the timer is now non-fatal on x86. |
| Domain starts then vanishes from `xl list` instantly, no console output; `sudo xl dmesg` shows `Error: Missing xen PVH initialization` then `Triple fault - invoking HVM shutdown` | Kernel has `CONFIG_PVH=y` (so `xl create` accepts the PVH note) but **not `CONFIG_XEN_PVH=y`** — the Xen-specific PVH guest init is the `__weak` stub that BUG()s. Rebuild with `CONFIG_XEN_PVH=y` (Section 3). To see early boot before `hvc0` is up, add `earlyprintk=xen` to `extra` and read `sudo xl dmesg`. |
| Guest boots but `xl console` is blank | Kernel lacks `CONFIG_HVC_XEN`/`CONFIG_HVC_XEN_FRONTEND`, so `console=hvc0` has no backend. Rebuild with those `=y`; confirm boot independently via `earlyprintk=xen` + `sudo xl dmesg`. |
| `xl create` fails: `xc_dom_find_loader: no loader found: Invalid kernel` | The kernel `Image` is the wrong architecture (e.g. an ARM64 kernel on x86). Ship an x86_64 bzImage. |
| `xl create` fails: `segment kernel too large … Out of memory` | Guest `memory` too small for the kernel. Raise `"memory"` in `/boot/config.json`. |
| `image's platform (linux/arm64) does not match host` then create fails | The container (and its kernel) are ARM64; they can't run on x86 Xen. Use x86_64 artifacts. |
| `xl: command not found` during create | Xen toolstack not installed / not in PATH for the docker service. |
| Container managed by runphi but should be plain (or vice-versa) | Forwarding keys off `/boot/config.json` in the rootfs. Add/remove it, or set the OCI annotation `org.runphi.runtime=runc`/`runphi`. |
| LVM create fails: `Volume group "test-vg" not found` | No VG with the expected name. Create one (`vgcreate`) or write your VG's name into `/usr/share/runPHI/xen_lvm_vg`. |
| Guest can't mount `/dev/xvda` (disk modes, no initramfs) | Kernel lacks built-in `xen-blkfront` (`CONFIG_XEN_BLKDEV_FRONTEND=y`), or ship a `ramdisk` initramfs that loads it as a module. |
| Stale `lv_<id>` volumes after crashes | Teardown removes them; if runphi died mid-create, clean manually: `sudo lvremove -y /dev/<vg>/lv_<id>`. |
