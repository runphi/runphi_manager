#!/bin/bash

## This script switch to vanilla runtime
## If runc and containerd vanilla are already running exit.

CWD="$HOME/target"

# setup FLANNEL
sh ${CWD}/setup_flannel.sh

echo "Three card games with runc..."
current_runtime_name=$(${CWD}/get_current_container_runtime.sh|awk '{print $1}')
if [[ "${current_runtime_name}" == "runc" ]]; then
	echo "Current runtime is already ${current_runtime_name}...skip!"
else
	echo "Current runtime is ${current_runtime_name}...restore runc!"
	cp /usr/local/sbin/runc_vanilla /usr/bin/runc
	echo "runc restored!!!"
fi

