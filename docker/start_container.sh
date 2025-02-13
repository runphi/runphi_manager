#!/bin/bash
docker build -t runphi_builder .
docker stop runphi_manager_compiler
docker rm runphi_manager_compiler
cd .. 
docker run -it -v ./rust_runphi:/home -w="/home" --name runphi_manager_compiler runphi_builder /bin/bash


