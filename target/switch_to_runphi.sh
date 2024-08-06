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
	cp /usr/bin/runc /usr/local/sbin/runc_vanilla
	echo "runc backed up"

	echo "Replace runc with runphi..."
	cp /root/runphi /usr/bin/runc
	echo "runc replace with runphi!"

else
	echo "Current runtime is already ${current_runtime_name}!"
fi

# backup runc_vnailla and move to local sbin. It will be called by forwarding by runphi
#if [ ! -f /usr/local/sbin/runc_vanilla ]; then 
#	cp /usr/bin/runc /usr/local/sbin/runc_vanilla
#	echo "Backup runc"
#fi

## Launch containerd patched for Jailhouse ARM64 with runphi config.toml
#/usr/bin/containerd_arm64jh --config /etc/containerd/containerd_runphi_config.toml --log-level info > /var/log/container.log 2>&1 &
# containerd with name equal to "/usr/bin/containerd_arm64jh" is our containerd patched...check if it's already running

current_containerd_name=$(${CWD}/get_current_container_manager.sh | awk '{print $1}')

if [[ ${current_containerd_name} == "/usr/bin/containerd_arm64jh" ]]; then 

	echo "containerd patched for Jailhouse ARM64 is already running do not reload it!"
	exit 0
else
	/etc/init.d/S91containerd stop
	echo "Launch containerd patched for Jailhouse ARM64 with containerd_runphi_config.toml..."
	cp ${CWD}/S91containerd_arm64jh /etc/init.d/S91containerd
	/etc/init.d/S91containerd start
fi
