# runPHI

<img src="https://dessert.unina.it:8088/runphi/runphi/-/raw/main/runPHI/logo/runphi_logo_lowres.jpg" width="100">

----

#### Authors: Marco Barletta, Francesco Boccola, Daniele Ottaviano, Luigi De Simone & other members of DESSERT lab UniNa
#### Università degli Studi di Napoli Federico II
#### marco.barletta@unina.it, francesco.boccola@unina.it
----

runPHI is a container RUNtime for Partitioning Hypervisor Integration. 
What that means? A container runtime is whatever seats below your containerd, docker, or kubelet, and is the tiny program in charge of setting up kernel stuff to start a container.

runPHI does something similar, but with partitioning hypervisors, which are hypervisors designed for industrial scenarios with critical requirements in mind.

runPHI allows you to take your favourite Zephyr, FreeRTOS, or even bare metal code, running on industrial hardware platforms (INCLUDING RPUs!!!!!) and integrate it with your container orchestration system (e.g., Kubernetes).
We call this isolated container ZICs (Zero-Interference Containers).

Why that? We envision an industrial cloud in which critical applications are seamlessly managed along with normal Linux containers, with the same flexbility, allowing you to define the number of replicas, deploying them across a large cluster, while keeping a high standards of non-functional requirements. 

runPHI is OCI compliant (at least, for basic calls like start, stop, kill, and delete). The runtime is still an academic research project, if you have any suggestion to improve it (or even better, you want to contribute!!!) please reach at the emails above.

## Repo Structure

- doc, with the current documentation
- logo, well, the name is self explanatory
- CNI, with a Container Network Interface to integrate the Kubernetes networking with the hypervisor one (it assumes having two VMs comunicating over network interfaces, still experimental)
- docker, with the files required to containerize the building process
- rust_runphi, with the actual source tree for the container runtime
- target, with files that are necessary to integrate runPHI in the environment built with our environment builder

## Build runPHI

If you want to build runPHI for aarch64, try: 

    cd rust_runphi
    ./compile_rust.sh 
    
That requires a rust toolchain. If you do not want to install one, use the docker script to run those commands in a container.

    docker/start_container.sh
    

## Debug runPHI

export RUNPHI_DEBUG_LEVEL={level}

where {level} can be "error", "warn", "info", "debug", "trace"

Export the variable in an environment such that containerd (or any other program that calls runPHI) can inherit the variable.
For example, export it and then from the shell restart containerd, or manually launch containerd.

runPHI logs by default in /usr/share/runPHI/log.txt
Note that that folder must be populated with other useful files. So you have to have it in your environment. The files are copied from the target directory in the root of this repo to the target environment by our environment builder. Look for more info there.

## RunPHI Architecture

RunPHI is written in rust, and divided in crates

- runphi: containing the main function of the program. It is hypervisor-independent. In order to make the ZICs visible to common tools, the creation of the pause container is forwarded to runc_vanilla to actually create a pause container.
- logging: a crate used by every other crate to handle logging in a systematic way.
- liboci_cli: to parse OCI command line arguments into data structures.
- frontend_to_backend: contains data structures that parse the config.json in /boot/ of the ZICs and other information from the frontend of runPHI into datastructures that work as APIs for the backend-part, which is hypervisor depedent. In other words, both the frontend and backend agree on the format of these data structures, and they both use them.
- backend_{backend_name}: a folder for each backend supported. At the moment, Jailhouse is the only stable and tested backend.
The backend usually contains:
- a config_generator, which drives the generation of configuration files for the partitioned container, relying on hypervisor-dependent backend
- resource files: called by the config_generator, managers of the available resources of the hardware platoform and called to create the config file
For Jailhouse there is a file for each of the follwoing resources: CPU, memory, RPUs, devices, communication, network, and other boot parameters. 
- template manager: the configuration is based on a template which is provided for the hardware platform. A file takes care of loading such templates.

The backend calls the resource managers and some helper functions (config_generator_helper_start, //_end ), and implements the functions to start, stop, delete, kill the ZIC, with implementation that strongly depends on the hyprevisor.

RunPHI generates some files for each ZIC to keep track of their information (like cell configuration file and compiled configuration). They can be found in /run/runPHI/<ContainerName>/


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


## Demo

**qemu-jailhouse** environment: refer to this [README](https://dessert.unina.it:8088/runphi/partitioned_container_demos/-/tree/main/demos).

## Issues and TODO
- the shim must be modified to incorporate caronte, and the management of the shim must be integrated (atm kill kills the shim)
- Moving distinguishing layer into shim?
- Implement real resource allocators to create and compile .c to .cell file on the fly. At the moment pre-made .cell files are used.
In several part of the code there are conditions to control whether OS is zephyr. Is not really useful, right now it is needed to set loading address. It should not be useful once you can create the .c file with custom memory addresses.
- When allotting a cpu, the inmate example may fail ... irq not corresponding to cpu????
- Rustify caronte



