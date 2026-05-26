#!/bin/bash

# List of files and directories to keep
KEEP=("Makefile" "state.toml" "initial_state.toml" "platform_info.toml" "cleanup.sh" "caronte" "include" "configuration")

# Working directories and files
TARGET_DIR="/usr/share/runPHI"
RUN_DIR="/run/runPHI"
STATE_FILE="$TARGET_DIR/state.toml"
INITIAL_STATE_FILE="$TARGET_DIR/initial_state.toml"

# Default options
CLEAN_CONTAINERS=false
RESTORE_STATE=false
PRUNE_CONTAINERS=false
ALL_CLEAN=false

# Function to show help
show_help() {
  echo "Usage: $0 [OPTIONS]"
  echo "Options:"
  echo "  -c, --clean         Remove all framework files in $RUN_DIR/"
  echo "  -r, --restore-state Restore state.toml from initial_state.toml"
  echo "  -p, --prune-containers Prune stopped Docker containers, networks, and images"
  echo "  -a, --all           Perform all cleanup operations"
  echo "  -h, --help          Display this help message"
}

# Parse command-line options
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    -c|--clean)
      CLEAN_CONTAINERS=true
      shift
      ;;
    -r|--restore-state)
      RESTORE_STATE=true
      shift
      ;;
    -p|--prune-containers)
      PRUNE_CONTAINERS=true
      shift
      ;;
    -a|--all)
      ALL_CLEAN=true
      shift
      ;;
    -h|--help)
      show_help
      exit 0
      ;;
    *)
      echo "Error: Invalid option $1"
      show_help
      exit 1
      ;;
  esac
done

# If --all is selected, enable all cleanup options
if $ALL_CLEAN; then
  CLEAN_CONTAINERS=true
  RESTORE_STATE=true
  PRUNE_CONTAINERS=true
fi

# Attempt to change to the desired directory
cd "$TARGET_DIR" || { 
  echo "Error: Could not change to directory $TARGET_DIR. Exiting."; 
  exit 1; 
}

# Enable dotglob so '*' matches hidden files (e.g., .env)
shopt -s dotglob

# Loop through all files and directories in the current directory
for item in *; do
  # Check if the item is not in the list of files to keep
  if [[ ! " ${KEEP[@]} " =~ " ${item} " ]]; then
    # Remove the item
    rm -rf "$item"
    # echo "Removed: $item"
  fi
done

# Disable dotglob to return to normal behavior
shopt -u dotglob

# Clean run directories if the option is set
if $CLEAN_CONTAINERS; then
  if [ -d "$RUN_DIR" ]; then
    rm -rf "${RUN_DIR:?}"/*
    echo "Removed all framework files in $RUN_DIR/"
  else
    echo "Directory $RUN_DIR does not exist. Skipping."
  fi
fi

# Prune Docker containers if the option is set
if $PRUNE_CONTAINERS; then
  echo "Cleaning Docker environment..."
  
  # Remove all stopped containers
  docker container prune -f
  
  # Remove custom networks not used by at least one container
  docker network prune -f
  
  # Remove dangling images
  docker image prune -f
  
  # Remove unused persistent volumes (uncomment to wipe testing data)
  # docker volume prune -f
  
  echo "Docker cleanup complete."
fi

# Restore state.toml from initial_state.toml if the option is set
if $RESTORE_STATE; then
  if [[ -f "$INITIAL_STATE_FILE" ]]; then
    cp "$INITIAL_STATE_FILE" "$STATE_FILE"
    echo "Restored state.toml from initial_state.toml."
  else
    # Check for Xen environment via sysfs/procfs (no external tools needed)
    if [[ -d "/proc/xen" ]] || grep -qi "xen" /sys/hypervisor/type 2>/dev/null; then
      echo "Info: Xen environment detected. No state restoration needed."
    else
      echo "Warning: initial_state.toml not found on non-Xen system. Cannot restore state."
    fi
  fi
fi

echo "Cleanup complete."