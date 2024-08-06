#!/bin/bash

# Define the path and file
FLANNEL_DIRECTORY="/run/flannel"
FLANNEL_CONFIG_FILE="${FLANNEL_DIRECTORY}/subnet.env"

# Create the directory if it doesn't exist
if [ ! -d "${FLANNEL_DIRECTORY}" ]; then
  mkdir -p "${FLANNEL_DIRECTORY}"
  # echo "Created directory: ${FLANNEL_DIRECTORY}"
fi

# Write content to the file
echo "FLANNEL_NETWORK=10.244.0.0/16" > "${FLANNEL_CONFIG_FILE}"
echo "FLANNEL_SUBNET=10.244.0.1/24" >> "${FLANNEL_CONFIG_FILE}"
echo "FLANNEL_MTU=1450" >> "${FLANNEL_CONFIG_FILE}"
echo "FLANNEL_IPMASQ=true" >> "${FLANNEL_CONFIG_FILE}"

echo "Created /run/flannel/subnet.env"
