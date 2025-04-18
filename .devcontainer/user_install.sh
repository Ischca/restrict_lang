#!/bin/bash

# Download rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Default installation, add additional components
~/.cargo/bin/rustup set profile default 
~/.cargo/bin/rustup self update 
~/.cargo/bin/rustup component add rls rust-analysis rust-src
~/.cargo/bin/rustup completions bash > ~/.rustup_completions_bash 
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc  
echo '. ~/.rustup_completions_bash' >> ~/.bashrc 

# Remove container hostname from prompt
sed -i 's|\(PS1=.*\@\)\\h|\1docker|g' ~/.bashrc 

# Set proper unix file permissions
chmod 0700 ~/. 
chmod 0640 ~/.bash*
chmod 0640 ~/.profile*

LLVM_VERSION=17
# install llvm
apt install lsb-release wget software-properties-common gnupg zlib1g-dev libpolly-$LLVM_VERSION-dev -y
# get newest llvm.sh
# wget https://apt.llvm.org/llvm.sh
sudo ./llvm.sh $LLVM_VERSION

# install wasmer
curl https://get.wasmer.io -sSfL | sh
source /home/vscode/.wasmer/wasmer.sh
