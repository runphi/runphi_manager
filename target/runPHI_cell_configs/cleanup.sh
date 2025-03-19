#!/bin/bash

# List of files and directories to keep
KEEP=("Makefile" "state.toml" "platform_info.toml" "cleanup.sh" "caronte" "include" "configuration")

# Working directory
TARGET_DIR="/usr/share/runPHI"
STATE_FILE="$TARGET_DIR/state.toml"

# Default options
CLEAN_CONTAINERS=false
RESTORE_STATE=false
PRUNE_CONTAINERS=false
ALL_CLEAN=false

# Function to show help
show_help() {
  echo "Usage: $0 [OPTIONS]"
  echo "Options:"
  echo "  -c, --clean         Remove all containers in /run/runPHI/"
  echo "  -r, --restore-state Restore state.toml to its original contents"
  echo "  -p, --prune-containers Remove all Docker containers in /var/lib/docker/containers/"
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

# Loop through all files and directories in the current directory
for item in *; do
  # Check if the item is not in the list of files to keep
  if [[ ! " ${KEEP[@]} " =~ " ${item} " ]]; then
    # Remove the item
    rm -rf "$item"
    # echo "Removed: $item"
  fi
done

# Clean containers if the option is set
if $CLEAN_CONTAINERS; then
  rm -rf /run/runPHI/*
  echo "Removed all containers in /run/runPHI/"
fi

# Prune Docker containers if the option is set
if $PRUNE_CONTAINERS; then
  rm -rf /var/lib/docker/containers/*
  echo "Removed all Docker containers in /var/lib/docker/containers/"
fi

# Restore state.toml if the option is set
if $RESTORE_STATE; then
  cat > "$STATE_FILE" << EOF
[containerid]
ids = []

[available_memory]
memory = "0x3ed00000, 0x46d00000"

[free_segments]
segments= ["0x3ed00000, 0x46d00000"]

[free_pci_devices_bdf]
bdf = [1,2]

[free_rcpus]
ids = [0]
EOF
  echo "Restored state.toml to original contents."
fi

echo "Cleanup complete."
