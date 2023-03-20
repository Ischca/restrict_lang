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
