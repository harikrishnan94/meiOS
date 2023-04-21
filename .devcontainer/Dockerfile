FROM ubuntu:latest

WORKDIR /root

# Locale Gen
ARG DEBIAN_FRONTEND=noninteractive
RUN apt update && apt install -y g++-12 gdb git python3-pip locales sudo \
    lsb-release wget software-properties-common gnupg

# Install cmake and ninja
RUN pip3 install cmake ninja

# Install LLVM
ARG LLVM_VER=16
RUN wget https://apt.llvm.org/llvm.sh && chmod +x llvm.sh && \
    sed -i 's/^add-apt-repository/add-apt-repository -y/g' llvm.sh && \
    ./llvm.sh ${LLVM_VER} all

# Create a 'dev' user
ARG DEV_USER_NAME=dev
RUN useradd ${DEV_USER_NAME} -d /home/${DEV_USER_NAME} && passwd -d ${DEV_USER_NAME} && \
    mkdir /home/${DEV_USER_NAME} && chown ${DEV_USER_NAME} /home/${DEV_USER_NAME} && \
    usermod -aG sudo ${DEV_USER_NAME} && \
    echo "$DEV_USER_NAME ALL=(ALL:ALL) NOPASSWD: ALL" | sudo tee /etc/sudoers.d/${DEV_USER_NAME}

RUN sed -i '/en_US.UTF-8/s/^# //g' /etc/locale.gen && \
    sed -i '/en_IN.UTF-8/s/^# //g' /etc/locale.gen && \
    sed -i '/ta_IN.UTF-8/s/^# //g' /etc/locale.gen && \
    locale-gen

RUN (echo '#!/usr/bin/env bash' && \
    echo 'echo bashit enable plugin alias-completion colors edit-mode-vi explain extract git gitstatus git-subrepo history-eternal' \
    'history history-search history-substring-search man projects sudo' -n \
    'echo bashit enable alias apt bash-it git tmux vim' -n \
    'echo bashit enable completion defaults export git github-cli makefile pip3 tmux') > /show-enable-bashit-common

# Set timezone
RUN apt update && \
    apt install -yq tzdata && \
    ln -fs /usr/share/zoneinfo/America/Kolkata /etc/localtime && \
    dpkg-reconfigure -f noninteractive tzdata

# Install aarch64-none-elf-gcc
ARG AARCH64_ELF_GCC_VER=12.2.rel1
RUN dpkgArch="$(dpkg --print-architecture)"; \
    case "$dpkgArch" in \
    arm64) export HOST_ARCH=aarch64;; \
    x86_64) export HOST_ARCH=x86_64;; \
    esac; \
    wget -c https://developer.arm.com/-/media/Files/downloads/gnu/${AARCH64_ELF_GCC_VER}/binrel/arm-gnu-toolchain-${AARCH64_ELF_GCC_VER}-${HOST_ARCH}-aarch64-none-elf.tar.xz && \
    tar xf arm-gnu-toolchain-${AARCH64_ELF_GCC_VER}-${HOST_ARCH}-aarch64-none-elf.tar.xz && \
    mv arm-gnu-toolchain-${AARCH64_ELF_GCC_VER}-${HOST_ARCH}-aarch64-none-elf /usr/local/aarch64-none-elf && \
    rm arm-gnu-toolchain-${AARCH64_ELF_GCC_VER}-${HOST_ARCH}-aarch64-none-elf.tar.xz
# Add to PATH
ENV PATH="$PATH:/usr/local/aarch64-none-elf/bin"
# Verify installation
RUN aarch64-none-elf-g++ -v