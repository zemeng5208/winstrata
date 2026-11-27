#!/usr/bin/env bash
set -euo pipefail

node_version=$(node --print 'require("./package.json").engines.node')
pnpm_pin=$(node --print 'require("./package.json").packageManager')
rust_version=$(awk -F '"' '/^channel = / {print $2}' rust-toolchain.toml)
version_pattern='^[0-9]+\.[0-9]+\.[0-9]+$'
pnpm_version=${pnpm_pin#pnpm@}
if [[ ! "$node_version" =~ $version_pattern || ! "$rust_version" =~ $version_pattern ||
      "$pnpm_pin" != pnpm@* || ! "$pnpm_version" =~ $version_pattern ]]; then
  printf '%s\n' 'The workspace must declare exact Rust, Node.js, and pnpm versions.' >&2
  exit 1
fi

toolchain="$RUNNER_TEMP/winstrata-toolchain"
download="$toolchain/download"
mkdir -p "$download" "$toolchain/node"
node_distribution="https://nodejs.org/dist/v$node_version"
archive="node-v$node_version-linux-x64.tar.xz"
curl --fail --silent --show-error --retry 3 --max-time 180 "$node_distribution/$archive" -o "$download/$archive"
curl --fail --silent --show-error --retry 3 --max-time 30 "$node_distribution/SHASUMS256.txt" -o "$download/SHASUMS256.txt"
(
  cd "$download"
  awk -v archive="$archive" '$2 == archive { print }' SHASUMS256.txt | sha256sum --check --strict
)
tar --extract --xz --file "$download/$archive" --strip-components=1 --directory "$toolchain/node"
export PATH="$toolchain/node/bin:$PATH"
npm install --global --ignore-scripts --no-audit --prefix "$toolchain" --cache "$RUNNER_TEMP/npm-cache" "pnpm@$pnpm_version"
rustup toolchain install "$rust_version" --profile minimal
printf '%s\n' "$toolchain/node/bin" "$toolchain/bin" >> "$GITHUB_PATH"
