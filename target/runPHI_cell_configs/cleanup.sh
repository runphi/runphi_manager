#!/bin/bash

# List of files and directories to keep
KEEP=("Makefile" "state.toml" "platform-info.toml" "cleanup.sh" "caronte" "include" "configuration")

# Working directory
TARGET_DIR="/usr/share/runPHI"

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

# Ask for confirmation before removing all containers in /run/runPHI/
read -p "Do you want to remove all containers in /run/runPHI/? (y/n) " -n 1 -r
echo    # move to a new line
if [[ $REPLY =~ ^[Yy]$ ]]; then
  rm -rf /run/runPHI/*
  echo "Removed all containers in /run/runPHI/"
else
  echo "Skipped removing containers in /run/runPHI/"
fi

echo "Cleanup complete."