#!/bin/bash

# Grab the directory of this script.
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

echo "Building binary..."
cargo build --release --bin cannon

# Check if `docker` is installed
if ! command -v docker &> /dev/null
then
    echo "Error: docker not found. Please install docker and try again."
    exit
fi

echo "Building image..."
docker build -f cannon-rs.dockerfile $DIR/.. -t cannon-rs
