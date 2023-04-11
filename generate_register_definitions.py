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


def generate_bool(field_name: str, num_bits: int, buffer: io.TextIOBase):
    if num_bits != 1:
        return

    write(buffer,
          f"[[no_unique_address]] Value<{field_name}_def, true, 1> SET;\n")
    write(
        buffer, f"[[no_unique_address]] Value<{field_name}_def, true, 0> CLEAR;\n")


def generate_enums(field_name: str, enums: dict, buffer: io.TextIOBase):
    for name, val in enums.items():
        write(buffer,
              f"[[no_unique_address]] Value<{field_name}_def, true, {val}> {name};\n")

    write(buffer, "enum class Enum : word_type {\n")
    increase_indent_level()

    for name, val in enums.items():
        write(buffer, f"{name} = {val},\n")

    decrease_indent_level()
    write(buffer, "};\n")

    write(
        buffer, f"[[nodiscard]] constexpr auto operator()(Enum e) const noexcept  {{\n")
    increase_indent_level()
    write(
        buffer, f"return Value<{field_name}_def, false, 0>{{static_cast<word_type>(e)}};\n")
    decrease_indent_level()
    write(buffer, "}\n")

    # Generate EnumStr()
    write(
        buffer, "[[nodiscard]] static constexpr auto EnumStr(word_type e) -> std::optional<std::string_view> {\n")

    increase_indent_level()
    write(buffer, "switch(e) {\n")

    increase_indent_level()
    for name, val in enums.items():
        write(buffer, f"case {val}: return \"{name}\";\n")
    write(buffer, "default: return {};\n")
    decrease_indent_level()

    write(buffer, "}\n")
    decrease_indent_level()

    write(buffer, "}\n")

    # Generate IsValid()
    write(
        buffer, "[[nodiscard]] static constexpr auto IsValid(word_type e) -> bool {\n")

    increase_indent_level()
    write(buffer, "switch(e) {\n")

    increase_indent_level()
    for name, val in enums.items():
        write(buffer, f"case {val}: return true;\n")
    write(buffer, "default: return false;\n")
    decrease_indent_level()

    write(buffer, "}\n")

    write(buffer, "return false;\n")
    decrease_indent_level()
    write(buffer, "}\n")


def generate_field(field, regname: str, buffer: io.TextIOBase) -> str:
    keys = list(field.keys())
    values = list(field.values())
    name = keys[0]

    if len(keys) > 1:
        rng = keys[1]
        offset, count = get_field_range(rng)
    else:
        offset, count = get_field_range(values[0])

    write(buffer,
          f"struct {name}_def : ::mei::registers::GenericField<{regname}, {offset}, {count}, \"{name}\"> {{\n")
    increase_indent_level()

    write(
        buffer, f"[[nodiscard]] constexpr auto operator()(word_type v) const noexcept  {{\n")
    increase_indent_level()
    write(buffer, f"return Value<{name}_def, false, 0>{{v}};\n")
    decrease_indent_level()
    write(buffer, "}\n")

    write(
        buffer, f"[[nodiscard]] constexpr auto ValInternalUse(word_type v) const noexcept  {{\n")
    increase_indent_level()
    write(buffer, f"return Value<{name}_def, false, 0>{{v >> {offset}}};\n")
    decrease_indent_level()
    write(buffer, "}\n")

    generate_bool(name, count, buffer)
    if len(keys) > 1:
        generate_enums(name, dict(values[1]), buffer)

    decrease_indent_level()
    write(buffer, "};\n")
    write(buffer, f"// Offset = {offset}, NumBits = {count}\n")
    write(buffer, f"[[no_unique_address]] {name}_def {name};\n")

    return name


def generate_register(name: str, type: str, system_name: str, fields: list, buffer: io.TextIOBase):
    write(buffer, f"namespace detail {{\n")
    increase_indent_level()

    write(buffer,
          f"struct {name} : ::mei::registers::GenericRegister<::ktl::{type}, \"{name}\"> {{\n")
    increase_indent_level()

    field_names = []
    for field in fields:
        field_name = generate_field(field, name, buffer)
        field_names.append(field_name)
        write(buffer, "\n")

    field_defs = [f"{name}_def" for name in field_names]
    write(
        buffer, f"using field_types = std::tuple<{','.join(field_defs)}>;\n")

    decrease_indent_level()
    write(buffer, "};\n")
    decrease_indent_level()
    write(buffer, "}\n")

    write(buffer, f"inline constexpr detail::{name} {name};\n")

    if system_name != None:
        write(buffer,
              f"DEFINE_SYSTEM_REGISTER({name}, detail::{name}, \"{system_name}\");\n")


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

#include <string_view>
#include <tuple>

#include <mei/registers.hpp>

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
