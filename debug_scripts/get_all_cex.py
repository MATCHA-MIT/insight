#!/usr/bin/env python3
from __future__ import annotations
import sys
import glob
import json
import shutil
import os

# -*- coding: utf-8 -*-
"""get_all_cex.py - entrypoint for collecting counterexamples (placeholder)."""

def get_md5hash(filepath: str) -> str:
    """Compute the MD5 hash of a file."""
    import hashlib
    hash_md5 = hashlib.md5()
    with open(filepath, "rb") as f:
        for chunk in iter(lambda: f.read(4096), b""):
            hash_md5.update(chunk)
    return hash_md5.hexdigest()


def main(invariant_path: str, out_path: str) -> int:
    """Main entrypoint. Replace with actual implementation."""
    counter = 0
    os.makedirs(out_path, exist_ok=True)
    for invariant_file in glob.glob(invariant_path + "/*.json"):
        print(f"\r Processing invariant file: {invariant_file}",end="")
        this_invariant = json.load(open(invariant_file, "r"))
        print(f"\r Loaded invariant: {this_invariant}")
        if "input_cex" in this_invariant:
            cex_filepath = this_invariant["input_cex"]["filepath"]
            print(f"\r Found counterexample at: {cex_filepath}",end ="")
            counter += 1
            md5hash = get_md5hash(cex_filepath)[:10]
            shutil.copy(cex_filepath, out_path + f"/cex_{counter}_{md5hash}.bin")
            print(f"\r Copied to: {out_path}/cex_{counter}_{md5hash}.bin", end="")
    print(f"\nTotal counterexamples collected: {counter}")
    return 0
            



if __name__ == "__main__":
    sys.exit(main(sys.argv[1], sys.argv[2]))
