#!/bin/bash
# Compile and extract clean WAT

cargo run --bin restrict_lang "$1" 2>&1 | 
  awk '/^\(module/{p=1} /^✓ Successfully compiled/{p=0} p' |
  sed '$d' > "$2"