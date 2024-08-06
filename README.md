# runPHI

<img src="https://dessert.unina.it:8088/runphi/runphi/-/raw/main/runPHI/logo/runphi_logo_lowres.jpg" width="100">

----

LOREM IPSUM

#### Authors: Marco Barletta & DESSERT
#### Università degli Studi di Napoli Federico II
#### marco.barletta@unina.it, other emails
----


## Structure

- runphi:       main runhpi code, runtime independent
- configure:    detect and install runPHI for the appropriate runtime available on board
- frontend:
    - commands:     OCI-runtime compliant commands called by runphi
        - kill, pause, start, create, delete, resume, state
    - intermediate:
        - config_generator:             drives the generation of configuration files for the partitioned container, relying on runtime-dependent backend
        - config_generator_resources:   called by the config_generator, managers of resources for creating the config file, runtime-independent half, call the backend correspondend
            - MEM/NET/DEV/BOOT/CPU/COMM_config -- Mem manages memory, NET networkng, DEV acess to Devices, BOOT manages the startup configuraton for the guest, CPU manages CPU resources, COMM
- backend:              containing a subfolder for each supported runtime
    - <runtime x>:      runtime x
        - create-guest      runtime-dependent code to start a partitioned container
        - start-guest       runtime-dependent code to start a created partitioning container
        - destroy-guest     runtime-dependent code to destroy a partinioned contaner
        - stop-guest        runtime-dependent code to suspend a partinioned contaner
        - mount             runtime-dependent code to mount part of the host filesystem n the guest
        - config_generator: runtime-dependent stuff for managing the resources of partitioned container
            - MEM/NET/DEV/BOOT/CPU/COMM_config -- Mem manages memory, NET networkng, DEV acess to Devices, BOOT manages the startup configuraton for the guest, CPU manages CPU resources, COMM manages communication channels between guests and hosts (shared memories, terminals, etc.)
            - config_generator_helper_start helper to do some initial runtme-dependent configurations
            - config_generator_helper_stop  helper to do some initial runtme-dependent configurations
        - built-in: kernels and related stuff -- TEMPORARY, TO DELETE

    - <runtime y>:             runtime y
        - ...

Example with containerd:
        - Contaner-related fles will be stored in: /run/runPHI/<ContainerName>/
            Files:
                - assignedCPUs                  contains the allocation vector of the CPUs
                - config<ContainerName>.conf    output of config generator
                - pidfile                       path of the file containing pid of init process
                - <ContainerName>.c                      runtime-dependent conversion of output of conig_generator
                - bundle                        path of the file containing OCI bundle
                - inmate                        path of the inmate executable
                - rootfs                        path of the rootfs on the host
                - linux                         exists if the container to be started is a linux container



## Workflow

RunPHI uses a file that is populated with ID of containers to understand which container command must be redirected to runc.
Some filter conditions are used to understand whether to forward or not.
RunPHI reads a file inside the container called /boot/configuration.json to understand boot parameters to create the configuration for the partitioned cell.
This file must point to binary, or to kernels required to start the system.
Flow:
    containerd calls runPHI
    runPHI parses the bundle given by upper layer and dispatches to the right OCI commands
    the OCI command is executed, and if required it calls the intermediate frontend layer (e.g. config generator)
        Example:
            OCI create generates the configuration file, then the mount backend utility, and then the create-guest backend utility
            The configuration file creation call in order the start_helper, then all of the frontend/backend resource managers, and finally the end_helper.
            With jailhouse no-linux, the create-guest call jailhouse cell create passing the cellfile, later jailhouse cell load and jailhouse cell create


## Usage

#### qemu-jailhouse environment

**Pre-requisite**: assume environment ``qemu-jailhouse`` is properly built by running ``configure_everything.sh`` script (see [here](https://dessert.unina.it:8088/runphi/runphi#how-to-use-the-repository)).

##### From host machine
```
# cd /PATH_TO_RUNPHI/runphi

## setup bridge (HOST)-tap (QEMU) interconnection. Check nternal params to change hardcoded IP, subnet, etc.
# ./scripts/qemu/setup_bridge_tap.sh

## start QEMU VM
./scripts/qemu/start_qemu.sh

## setup QEMU VM as worker node in a k8s cluster. Check internal params to change control plane IP, QEMU IP, etc.
# ./scripts/orchestration/cluster_setup.sh

## Get the latest runphi binary and copy from host machine into QEMU VM (assume root user and 192.0.3.76 IP, change accordingly)

# cd /PATH_TO_RUNPHI/runPHI/rust_runphi
# ./compile_rust.sh
# scp /PATH_TO_RUNPHI/runPHI/rust_runphi/target/aarch64-unknown-linux-gnu/release/runphi root@192.0.3.76:/root/.
```

##### From QEMU VM (k8s worker node)
```
## Start Jailhouse and root cell (for Zephyr DEMO the root cell is in jailhouse/configs/arm64/qemu-arm64-zephyr-rootcell.cell)
## Check all available .cell file in jailhouse/configs dir

# cd scripts_jailhouse_qemu/
# sh start_jailhouse_net.sh qemu-arm64-zephyr-rootcell.cell
```

##### From control plane node

Create pod .yaml manifest (e.g., ``helloworld.yaml``) as in the following:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: hello
spec:
  terminationGracePeriodSeconds: 0
  containers:

  - name: zephyr
    image: dessertunina/helloworld:1.0
    # Guarantee that we pull again the refreshed image since the tag 1.0 is not latest
    # latest guarantees always the pull of the most recent image. The others do not
    imagePullPolicy: Always

  nodeName: buildroot
```

The image set must be built following the instrucitons in this [repo](https://dessert.unina.it:8088/runphi/partitioned_container_demos). Then apply/delete manifest to start/remove partitioned container:

```
# kubectl apply -f helloworld.yaml
# kubectl delete -f helloworld.yaml
```

## Issues and TODO
- the shim must be modified to incorporate caronte, and the management of the shim must be integrated (atm kill kills the shim)
- Moving distinguishing layer into shim?
- Implement real resource allocators to create and compile .c to .cell file on the fly. At the moment pre-made .cell files are used.
In several part of the code there are conditions to control whether OS is zephyr. Is not really useful, right now it is needed to set loading address. It should not be useful once you can create the .c file with custom memory addresses.
- When allotting a cpu, the inmate example may fail ... irq not corresponding to cpu????
- Rustify caronte



