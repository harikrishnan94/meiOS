#!/usr/bin/env python3

import io
import yaml
import sys
from pathlib import Path

input_base_directory = sys.argv[1]

current_indent_level = 0


def increase_indent_level():
    global current_indent_level
    current_indent_level += 1


def decrease_indent_level():
    global current_indent_level
    current_indent_level -= 1


def write(buffer: io.TextIOBase, data: str):
    global current_indent_level

    buffer.write('\t' * current_indent_level)
    buffer.write(data)


def parse_range(rng_str: str) -> tuple[int, int]:
    rng = rng_str.split(',')
    return (int(rng[0]), int(rng[1]))


def get_field_range(value) -> tuple[int, int]:
    if type(value) == int:
        return (int(value), 1)
    else:
        if type(value) == str:
            return parse_range(value)
        else:
            rng = list(value[0].keys())[0]
            return parse_range(rng)


def generate_bool(num_bits: int, buffer: io.TextIOBase):
    if num_bits != 1:
        return

    write(buffer,
          f"static constexpr auto SET = Value{{1}};\n")
    write(buffer, f"static constexpr auto CLEAR = Value{{0}};\n")


def generate_enums(enums: dict, buffer: io.TextIOBase):
    for name, val in enums.items():
        write(buffer,
              f"static constexpr auto {name} = Value{{{val}}};\n")


def generate_field(field, buffer: io.TextIOBase):
    keys = list(field.keys())
    values = list(field.values())
    name = keys[0]

    if len(keys) > 1:
        rng = keys[1]
        offset, count = get_field_range(rng)
    else:
        offset, count = get_field_range(values[0])

    write(buffer,
          f"struct {name}: ::mei::registers::Field<\"{name}\", Register, {offset}, {count}> {{\n")
    increase_indent_level()

    generate_bool(count, buffer)
    if len(keys) > 1:
        generate_enums(dict(values[1]), buffer)

    decrease_indent_level()
    write(buffer, "};\n")


def generate_register(name: str, type: str, system_name: str, fields: list, buffer: io.TextIOBase):
    write(buffer, f"namespace {name} {{\n")
    increase_indent_level()

    write(buffer,
          f"struct Register: ::mei::registers::Register<::mei::{type}, \"{name}\"> {{\n")
    increase_indent_level()

    for field in fields:
        generate_field(field, buffer)
        write(buffer, "\n")

    decrease_indent_level()
    write(buffer, "};\n")

    if system_name != None:
        write(buffer,
              f"DEFINE_SYSTEM_REGISTER({name}, {name}::Register, \"{system_name}\");\n")

    decrease_indent_level()
    write(buffer, "}\n\n")


def generate_namespace(nsp: str, registers: list, buffer: io.TextIOBase):
    write(buffer, f"namespace {nsp} {{\n")
    increase_indent_level()

    for register in registers:
        register = register.get('register')
        name = register.get('name')
        type = register.get('type')
        system_name = register.get('system_name')
        fields = list(register.get('fields'))
        generate_register(name, type, system_name, fields, buffer)
        write(buffer, "\n")

    decrease_indent_level()
    write(buffer, "}\n\n")


for input_yaml in sys.argv[2:]:
    with open(f"{input_base_directory}/{input_yaml}", 'r') as defs:

        current_indent_level = 0
        defs = yaml.safe_load(defs)
        buffer = io.StringIO()

        output_file_header = f"""
// Generated by {sys.argv[0]} from {input_yaml}. DONOT EDIT THIS FILE.
#pragma once

#include "mei/register/access.h"
#include "mei/register/field.h"
#include "mei/register/register.h"

"""

        write(buffer, output_file_header)

        for namespace in defs.get('namespaces'):  # type: ignore
            namespace = namespace.get('namespace')
            name = namespace.get('name')
            regs = list(namespace.get('registers'))
            generate_namespace(name, regs, buffer)
            write(buffer, "\n")

        output_cpp = str(defs.get('output'))  # type: ignore
        Path(output_cpp).parent.mkdir(exist_ok=True, parents=True)
        if output_cpp != None:
            with open(output_cpp, 'w') as output:
                output.write(buffer.getvalue())
