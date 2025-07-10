#!/bin/bash

# Compile the test file
cargo run test/simple_add.rl test/simple_add.wat

# Convert WAT to WASM using wat2wasm (from wabt toolkit)
if command -v wat2wasm &> /dev/null; then
    wat2wasm test/simple_add.wat -o test/simple_add.wasm
    echo "Generated test/simple_add.wasm"
    
    # Run the WASM with wasmtime
    if command -v wasmtime &> /dev/null; then
        echo "Running WASM with wasmtime:"
        wasmtime test/simple_add.wasm
        echo "Exit code: $?"
    else
        echo "wasmtime not found. Install it with: curl https://wasmtime.dev/install.sh -sSf | bash"
    fi
else
    echo "wat2wasm not found. Install wabt toolkit first."
fi