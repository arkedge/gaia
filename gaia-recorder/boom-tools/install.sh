#!/bin/bash -Cue

export BINSTALL_VERSION="v1.10.14"
export JRSONNET_VERSION="v0.5.0-pre96-test"

curl -L --proto '=https' --tlsv1.2 -sSf "https://raw.githubusercontent.com/cargo-bins/cargo-binstall/${BINSTALL_VERSION}/install-from-binstall-release.sh" | env BINSTALL_VERSION=${BINSTALL_VERSION} CARGO_HOME=$(pwd) bash

./bin/cargo-binstall --root . tmtc-c2a               --version 1.1.1 --no-confirm --force

./bin/cargo-binstall --root . tlmcmddb-cli           --version 2.6.1 --no-confirm --force
./bin/cargo-binstall --root . kble                   --version 0.4.2 --no-confirm --force
./bin/cargo-binstall --root . kble-c2a               --version 0.4.2 --no-confirm --force

## install jrsonnet
arch=$(uname -m)
if [ "$arch" = "x86_64" ]; then
  arch="amd64"
fi
os=$(uname -s | tr -s '[:upper:]' '[:lower:]')
curl -L "https://github.com/CertainLach/jrsonnet/releases/download/${JRSONNET_VERSION}/jrsonnet-linux-${arch}" -o ./bin/jrsonnet
chmod +x ./bin/jrsonnet

## Build and install gaia-recorder from local source
BOOM_TOOLS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Find gaia-recorder root by looking for Cargo.toml with name = "gaia-recorder"
find_gaia_recorder_root() {
    local dir="$1"
    while [ "$dir" != "/" ]; do
        if [ -f "$dir/Cargo.toml" ] && grep -q 'name = "gaia-recorder"' "$dir/Cargo.toml" 2>/dev/null; then
            echo "$dir"
            return 0
        fi
        dir="$(dirname "$dir")"
    done
    return 1
}

GAIA_RECORDER_ROOT=$(find_gaia_recorder_root "$BOOM_TOOLS_DIR")

if [ -z "$GAIA_RECORDER_ROOT" ]; then
    echo "Error: Could not find gaia-recorder root directory"
    exit 1
fi

echo "Building gaia-recorder from: $GAIA_RECORDER_ROOT"
cd "$GAIA_RECORDER_ROOT"
cargo build --release
cp target/release/gaia-recorder "$BOOM_TOOLS_DIR/bin/gaia-recorder"
