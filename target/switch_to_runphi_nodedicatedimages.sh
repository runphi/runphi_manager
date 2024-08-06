#!/bin/bash


## Note: this script assumes that the vanilla containerd with standard configuration is in
## place. If this is not the case, please set them properly.

## With this script, we only replace the low level runtime, but we keep on pulling
## arm64 images. This means that we can potentially run also standard container, 
## assuming that there is a metalayer in runphi that detects when to forward to runc
## and when to not

## This version does not allow the seamless migration (unless both partitioned container image
## and standard contaienr image are packed together)

# setup FLANNEL
sh setup_flannel.sh

echo "Three card games with runc"
# backup runc_vnailla and move to local sbin. It will be called by forwarding by runphi
if [ ! -f /usr/local/sbin/runc_vanilla ]; then 
	cp /usr/bin/runc /usr/local/sbin/runc_vanilla
	echo "Backup runc"
fi
cp /root/runphi /usr/bin/runc
echo "Replaced runc with runphi"


