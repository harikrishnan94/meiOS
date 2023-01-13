#!/usr/bin/env python3

from ruamel.yaml import YAML
import re
import sys

input_yaml = sys.argv[1]

yaml = YAML()
nsp_regex = re.compile('^[a-zA-Z_]+[a-zA-Z_0-9]*$')


def is_valid_data_type(dt: str) -> bool:
    return ["u8", "u16", "u32", "u64"].count(dt) != 0


def is_valid_namespace(nsp: str) -> bool:
    return nsp_regex.search(nsp) != None


with open(input_yaml, 'r') as defs:
    defs = yaml.load(defs)

    output_cpp = defs["output"]

    for key, value in defs.items():
        if key == "output":
            continue

        print(key)
