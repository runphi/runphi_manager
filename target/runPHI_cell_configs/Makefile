### TODO: invoke directly gcc command to speedup compilation time

CC = gcc
OBJCOPY = objcopy
CFLAGS = -Werror -Wall -Wextra -D__LINUX_COMPILER_TYPES_H -I./include
OBJCOPYFLAGS = -O binary --remove-section=.note.gnu.property

# List all .c files in the current directory
SRC_FILES := $(wildcard *.c)

# Create a list of corresponding .o files
OBJ_FILES := $(SRC_FILES:.c=.o)

# Create a list of corresponding .cell files
CELL_FILES := $(OBJ_FILES:.o=.cell)

# Rule to compile .c to .o
%.o: %.c
	$(CC) $(CFLAGS) -c $< -o $@

# Rule to create .cell from .o
%.cell: %.o
	$(OBJCOPY) $(OBJCOPYFLAGS) $< $@

# Default target: build all .cell files
all: $(CELL_FILES)

# Clean up generated files
clean:
	rm -f $(OBJ_FILES) $(CELL_FILES)

# Prevent deletion of .o files
.PRECIOUS: $(OBJ_FILES)

.PHONY: all clean
