#!/bin/bash

runtime=$(/usr/bin/runc --version)
runtime_name=$(echo $runtime | awk '{print $1}')

if [[ "${runtime_name}" == "runphi" ]]; then

	runtime_version=$(echo $runtime | awk '{print $2}')

elif [[ "${runtime_name}" == "runc" ]]; then

	runtime_version=$(echo $runtime | awk '{print $3}')

fi

echo "${runtime_name} v${runtime_version} is running..."
