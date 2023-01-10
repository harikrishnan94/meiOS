#!/usr/bin/env python3

import sys
import subprocess
import json

# Path to c++ compiler.
gcc = sys.argv[1]
compile_commands_file_input = sys.argv[2]
compile_commands_file_updated = sys.argv[3]

command = [gcc, "-xc++", "-std=c++20", "-fsyntax-only", "-v", "-"]

print(f"{' '.join(command)}")
stdout = subprocess.check_output([gcc, "-xc++", "-std=c++20",
                                  "-fsyntax-only", "-v", "-"], input=b"",
                                 stderr=subprocess.STDOUT).decode('utf-8')
print(stdout)

matched = False
include_dirs = []
for line in stdout.splitlines():
    if line == '#include <...> search starts here:':
        matched = True
    if line == 'End of search list.':
        matched = False

    if matched and not line.startswith('#'):
        include_dirs.append(f"-I{line.strip()}")

with open(compile_commands_file_input, 'r') as compile_commands:
    new_compile_commands = []
    for compile_command in json.load(compile_commands):
        command = compile_command['command'].split()
        new_command = []
        added = False

        for arg in command:
            if arg.startswith('-I') and not added:
                new_command = new_command + include_dirs
                added = True
            new_command.append(arg)

        compile_command['command'] = ' '.join(new_command)
        new_compile_commands.append(compile_command)

        with open(compile_commands_file_updated, 'w') as compile_commands:
            json.dump(new_compile_commands, compile_commands, indent=2)
