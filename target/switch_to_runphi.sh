#!/bin/bash

## This script switch to runphi runtime, i.e., run /root/runphi and containerd patched for jailhouse
## If runphi and containerd patched are already running exit.


## With this script, we only replace the low level runtime, but we keep on pulling
## arm64 images. This means that we can potentially run also standard container, 
## assuming that there is a metalayer in runphi that detects when to forward to runc
## and when to not

CWD="$HOME/target"

# setup FLANNEL
sh ${CWD}/setup_flannel.sh

echo "Three card games with runc..."
current_runtime_name=$(${CWD}/get_current_container_runtime.sh|awk '{print $1}')
if [[ "${current_runtime_name}" == "runc" ]]; then
	echo "Current runtime is ${current_runtime_name}...backup it"
	mkdir -p /usr/local/sbin/

	cp /usr/bin/runc /usr/local/sbin/runc_vanilla
	echo "runc backed up"

	echo "Replace runc with runphi..."
	cp /root/runphi /usr/bin/runc
	echo "runc replace with runphi!"

else
	echo "Current runtime is already ${current_runtime_name}!"
fi

