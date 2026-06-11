# Linux Xen domU demo container.
#
# Ships a Linux kernel Image and a rootfs.cpio.gz initramfs under /boot. runPHI
# boots this as a PVH Linux domU (on x86) with the initramfs as the root
# filesystem (root in RAM, no virtual disk). See target/demo_containers/linux/
# config.json for the boot parameters and doc/README.md for the contract.
#
# NOTE: the Image / rootfs.cpio.gz in this directory are ARM64 artifacts. To run
# on x86 + Xen, replace them with an x86_64 kernel Image and an x86_64 initramfs
# before building.
FROM scratch

COPY Image /boot/Image
COPY rootfs.cpio.gz /boot/rootfs.cpio.gz
COPY config.json /boot/

LABEL org.opencontainers.image.os.features="xen"

WORKDIR /boot

CMD ["sh"]
