#!/bin/bash

# Output file name
output_file="tests/program_triple_write.bin"

# Clear or create the output file
> "$output_file"

# Example variable containing hex strings, one per line
# # https://riscvasm.lucasteske.dev/#
#hex_data=$(cat <<EOF
#800002b7
#08028293
#0002a303
#EOF
#)

hex_data=$(cat <<EOF
0e800113
80c10b13
ff828f93
EOF
)

#00100093
#f120a0f3
#EOF
#)
#300020f3
#3e800093
#3e800093
#EOF
#)

# Function to write a hex string in little-endian format to the binary file
write_hex_little_endian() {
    local hex_string="$1"
    # Split the hex string into 2-byte chunks, reverse them, and convert to binary
    echo "$hex_string" | sed 's/.\{2\}/& /g' | awk '{for (i=4; i>=1; i--) printf "%s", $i}' | xxd -r -p >> "$output_file"
}

# Function to write a hex string to the binary file
write_hex() {
    local hex_string="$1"
    echo -n "$hex_string" | xxd -r -p >> "$output_file"
}

# Loop through each line in the hex_data variable
while IFS= read -r hex_string; do
    # Skip empty lines (if any)
    if [[ -z "$hex_string" ]]; then
        continue
    fi

    # Validate the hex string (only valid hex characters and even length)
    if ! [[ "$hex_string" =~ ^[0-9a-fA-F]+$ ]] || (( ${#hex_string} % 2 != 0 )); then
        echo "Error: Invalid hex string '$hex_string'. Must be even-length and contain only [0-9a-fA-F]."
        exit 1
    fi

    # Write the valid hex string to the binary file
    write_hex_little_endian "$hex_string"
done <<< "$hex_data"

echo "Binary file '$output_file' created successfully!"
