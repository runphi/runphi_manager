#!/bin/bash

#CONTROL_PLANE_IP="192.168.100.21"
#join_cmd=$(ssh root@${CONTROL_PLANE_IP} "/root/create_token.sh" 2>/dev/null)
#echo "Waiting for the node to join"
#sleep 15
#ssh root@${IP_QEMU} "/etc/init.d/S92kubelet start"

## Load modules needed by kubelet
modprobe br_netfilter
echo 1 > "/proc/sys/net/ipv4/ip_forward"
$HOME/target/setup_flannel.sh
