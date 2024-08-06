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

## Launch containerd vanilla with default config.toml
# containerd with name equal to "/usr/bin/containerd" is vanilla containerd...check if it's already running

current_containerd_name=$(${CWD}/get_current_container_manager.sh | awk '{print $1}')

if [[ ${current_containerd_name} == "/usr/bin/containerd" ]]; then 

	echo "containerd vanilla is already running do not reload it!"
	exit 0
else
	/etc/init.d/S91containerd stop
	echo "Launch containerd vanilla with /etc/containerd/containerd_default_config.toml configuration..."
	cp ${CWD}/S91containerd_vanilla /etc/init.d/S91containerd
	/etc/init.d/S91containerd start
fi
