#!/usr/bin/env bash

bin="$1"
shift 1
bin_name=$(basename "$bin")
build_dir=$(dirname "$bin")
kernel_img="$build_dir"/kernel8.img
profile=$(cat $(find "$build_dir" -type f -name 'profile'))
qemu_command="qemu-system-aarch64 -M raspi3b -serial stdio -semihosting -kernel '${kernel_img}' $@"

cargo objdump --"$profile" -- -D > "$build_dir"/${bin_name}.s && \
    cargo objcopy --"$profile" -- -O binary "$kernel_img" > /dev/null && \
    echo "${qemu_command}" && eval "${qemu_command}"
