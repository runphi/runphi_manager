################################################################################################################
#                                                                                                              #
#                                                      PPPPPPPPPPPPPPPPP   HHHHHHHHH     HHHHHHHHHIIIIIIIIII   #
#                                                      P::::::::::::::::P  H:::::::H     H:::::::HI::::::::I   #
#                                                      P::::::PPPPPP:::::P H:::::::H     H:::::::HI::::::::I   #
#                                                      PP:::::P     P:::::PHH::::::H     H::::::HHII::::::II   #
# rrrrr   rrrrrrrrr   uuuuuu    uuuuuunnnn  nnnnnnnn      P::::P     P:::::P  H:::::H     H:::::H    I::::I    #
# r::::rrr:::::::::r  u::::u    u::::un:::nn::::::::nn    P::::P     P:::::P  H:::::H     H:::::H    I::::I    #
# r:::::::::::::::::r u::::u    u::::un::::::::::::::nn   P::::PPPPPP:::::P   H::::::HHHHH::::::H    I::::I    #
# rr::::::rrrrr::::::ru::::u    u::::unn:::::::::::::::n  P:::::::::::::PP    H:::::::::::::::::H    I::::I    #
#  r:::::r     r:::::ru::::u    u::::u  n:::::nnnn:::::n  P::::PPPPPPPPP      H:::::::::::::::::H    I::::I    #
#  r:::::r     rrrrrrru::::u    u::::u  n::::n    n::::n  P::::P              H::::::HHHHH::::::H    I::::I    #
#  r:::::r            u::::u    u::::u  n::::n    n::::n  P::::P              H:::::H     H:::::H    I::::I    #
#  r:::::r            u:::::uuuu:::::u  n::::n    n::::n  P::::P              H:::::H     H:::::H    I::::I    #
#  r:::::r            u:::::::::::::::uun::::n    n::::nPP::::::PP          HH::::::H     H::::::HHII::::::II  #
#  r:::::r             u:::::::::::::::un::::n    n::::nP::::::::P          H:::::::H     H:::::::HI::::::::I  #
#  r:::::r              uu::::::::uu:::un::::n    n::::nP::::::::P          H:::::::H     H:::::::HI::::::::I  #
#  rrrrrrr                uuuuuuuu  uuuunnnnnn    nnnnnnPPPPPPPPPP          HHHHHHHHH     HHHHHHHHHIIIIIIIIII  #
#                                                                                                              #
################################################################################################################

##
# Authors: Marco Barletta & DESSERT
# Università degli Studi di Napoli Federico II
# marco.barletta@unina.it, other emails
##

This subdir implements the runPHI project.
[1] Papers
[2]

Structure:
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

Usage:
1) Enable/Start the runtime on the target system -- Runtime-dependent
        E.G. In jailhouse run start_jailhouse.sh
        export PATH=$PATH:/root/jailhouse/tools/     To use tools from jailhouse without full path

2) Run configure to copy the backend to the runPHI path (/usr/share/runPHI)
        Flags:
        -c <RUNTIME> Configure for the RUNTIME runtime -- Supported runtimes: 'docker', 'jailhouse', 'xen'
        -a Autodetect container runtime. If multiple available, falls back on the first available among: 'docker', 'jailhouse', 'xen'

    configure copies runPHI in /usr/sbin/runc, and backups the vanilla runc to /usr/sbin/runc_vanilla

3) Use the container runtime to trgger runPHI
        Example with containerd:
        - Start containerd
            containerd &
        - Pull images with ctr. ctr is the CLI tool to communicate with containerd
            ctr i pull -k docker.io/library/ubuntu:latest
        - Run container
            ctr run --rm <image> <ContainerName>    -- E.G. ctr run --rm docker.io/library/ubuntu:latest testName
            ctr run -d -t --env INMATE=/root/jailhouse/inmates/demos/arm64/gic-demo.bin  docker.io/library/ubuntu:latest test /bin/sh
            ctr run -d --env LINUX=1  docker.io/library/ubuntu:latest qemu-arm64-linux-demo /bin/sh
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
        - To kill use: ctr task pause <containerName>
        - Issues atm: - ctr c rm returns that container is not stopped, probably related to state command
                      - Containerd has a part strictly related to runc, must be modified
                      - ctr t pause does not work consistently

Flow:
    containerd calls runPHI
    runPHI parses the bundle given by upper layer and dispatches to the right OCI commands
    the OCI command is executed, and if required it calls the intermediate frontend layer (e.g. config generator)
        Example:
            OCI create generates the configuration file, then the mount backend utility, and then the create-guest backend utility
            The configuration file creation call in order the start_helper, then all of the frontend/backend resource managers, and finally the end_helper.
            With jailhouse no-linux, the create-guest call jailhouse cell create passing the cellfile, later jailhouse cell load and jailhouse cell create



