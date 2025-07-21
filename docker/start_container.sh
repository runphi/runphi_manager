#!/bin/bash
SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
PARENT_DIR="$(dirname "$SCRIPT_DIR")"
docker build -t runphi_builder ${SCRIPT_DIR}
docker stop runphi_manager_compiler
docker rm runphi_manager_compiler
docker run -it -v ${PARENT_DIR}/rust_runphi:/home --name runphi_manager_compiler runphi_builder /bin/bash
