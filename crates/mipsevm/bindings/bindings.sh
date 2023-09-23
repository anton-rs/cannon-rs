#!/bin/bash

# This script is used to generate the bindings for the MIPS contracts in the
# Optimism monorepo.

# The current directory relative to the script.
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

# Check if a folder with relative path `../optimism` exists and is not empty.
# If it doesn't exist, install the submodules.
if [ ! -d "$DIR/optimism"  ] || [ -z "$(ls -A $DIR/optimism)" ]; then
    echo "Error: Optimism monorepo not present. Initializing submodules..."
    git submodule update --init --recursive
fi

# Check if `forge` is installed
if ! command -v forge &> /dev/null
then
    echo "Error: forge not found. Please install forge and try again."
    exit
fi

CTB="$DIR/optimism/packages/contracts-bedrock"

cd $CTB && \
    forge install && \
    forge build

MIPS_ARTIFACT="$CTB/forge-artifacts/MIPS.sol/MIPS.json"
PREIMAGE_ARTIFACT="$CTB/forge-artifacts/PreimageOracle.sol/PreimageOracle.json"

MIPS_BIN=$(cat $MIPS_ARTIFACT | jq -r '.bytecode.object')
PREIMAGE_DEPLOYED_BIN=$(cat $PREIMAGE_ARTIFACT | jq -r '.deployedBytecode.object')

echo "Removing old bindings..."
rm $DIR/*.bin
echo "Old bindings removed."

echo -n "${MIPS_BIN:2}" > $DIR/mips_creation.bin
echo -n "${PREIMAGE_DEPLOYED_BIN:2}" >> $DIR/preimage_oracle_deployed.bin

echo "Bindings generated successfully."
