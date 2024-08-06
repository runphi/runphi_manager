#!/bin/bash

container_manager_name=$(ps aux|grep "/usr/bin/containerd.*_"|grep -v grep|awk '{print $3}')

# NOTE: Currently, /usr/bin/containerd_arm64jh is the name for containerd patched for runphi

echo "${container_manager_name} is running..."
