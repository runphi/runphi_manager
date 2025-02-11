#!/bin/bash

# build image for building rust runphi
docker build -t rust_runphi_builder .

# compile rust runphi
docker run -it --rm --name compile_rust_runphi -v ${PWD}:/home -w="/home" rust_runphi_builder cargo build --release --target aarch64-unknown-linux-gnu
