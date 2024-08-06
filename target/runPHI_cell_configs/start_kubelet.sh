#!/bin/bash

### The kubelet plugin FLANNEL should automatically create file configs.
### Due to kernel issues, FLANNEL configs are created manually

# Start the kubelet service
/etc/init.d/S92kubelet start
# echo "Started kubelet service."
