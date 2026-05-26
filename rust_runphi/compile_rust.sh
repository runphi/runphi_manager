#!/bin/bash
# Cross-compile runphi for aarch64 with a selected hypervisor backend.
#
# Usage:
#   ./compile_rust.sh                 # default: jailhouse
#   ./compile_rust.sh jailhouse
#   ./compile_rust.sh xen
set -euo pipefail

BACKEND="${1:-jailhouse}"

case "$BACKEND" in
    jailhouse|xen) ;;
    *)
        echo "error: unknown backend '$BACKEND' (expected: jailhouse | xen)" >&2
        exit 1
        ;;
esac

echo "Building runphi for aarch64 with backend: $BACKEND"
exec cargo build \
    --release \
    --target aarch64-unknown-linux-gnu \
    -p runphi \
    --no-default-features \
    --features "$BACKEND"
