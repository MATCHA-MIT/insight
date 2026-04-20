#!/usr/bin/env python3
import json
import subprocess
from tqdm import tqdm

def process_json_file(json_file_path):
    # Load the JSON file
    with open(json_file_path, 'r') as file:
        data = json.load(file)

    # Get the list of entries
    bex_entries = data.get('bex', [])

    # Process each entry with a progress bar
    for entry in bex_entries: #, desc="Processing", unit="entry"):
        file_path = entry.get('file')
        file_source = entry.get('file_source')

        if file_source == "FileSource.Mutations":
            # Call the disassemble script
            subprocess.run(["./util_scripts/disassemble_objdump.sh", file_path])

if __name__ == "__main__":
    import sys

    if len(sys.argv) != 2:
        print("Usage: python script.py <json_file>")
        sys.exit(1)

    json_file_path = sys.argv[1]
    process_json_file(json_file_path)

