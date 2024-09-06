<h1 align="center">
<img src="./assets/banner.png" alt="Cannon" width="100%" align="center">
</h1>

<h4 align="center">
    An alternative implementation of the OP Stack's <a href="https://github.com/ethereum-optimism/optimism/tree/develop/cannon">Cannon</a> in Rust.
</h4>

<p align="center">
  <a href="https://github.com/clabby/cannon-rs/actions/workflows/ci.yaml">
    <img src="https://github.com/clabby/cannon-rs/actions/workflows/ci.yaml/badge.svg?label=ci" alt="Ci">
  </a>
  <img src="https://img.shields.io/badge/License-MIT-green.svg?label=license" alt="License">
  <a href="https://github.com/ethereum-optimism/monorepo"><img src="https://img.shields.io/badge/OP%20Stack-monorepo-red" alt="OP Stack"></a>
  <a href="https://t.me/+2yfSX0YikWMxNTRh"><img src="https://img.shields.io/badge/Telegram-x?logo=telegram&label=anton-rs%20contributors"></a>
</p>

<p align="center">
  <a href="#whats-a-cannon">What's a Cannon?</a> •
  <a href="#overview">Overview</a> •
  <a href="#credits">Credits</a> •
  <a href="#benchmarks">Benchmarks</a> •
  <a href="#contributing">Contributing</a> •
  <a href="#documentation">Documentation</a> •
  <a href="#docker">Docker</a>
</p>

## What's a Cannon?

Cannon is an emulator designed to simulate a single MIPS thread context on the EVM. Its primary use is to execute the [`op-program`][op-program]
(also known as the fault-proof program) for the [OP Stack][monorepo]'s interactive dispute protocol. The `op-program` consists
of a stripped down version of `op-geth`'s state transition code in addition to the derivation pipeline, and produces deterministic results.
Subsequently, it is compiled to MIPS to be ran on top of Cannon on-chain to prove fault in claims about the state of L2 on L1. Cannon also has a
native implementation of the MIPS thread context that mirrors the on-chain version, which enables the [op-challenger][op-challenger] to generate
state commitments for an `op-program` execution trace and participate in dispute games.

*TL;DR:*
* It's Rust code
* ...that was [originally Go code][cannon]
* ...that runs an EVM
* ...emulating a MIPS machine
* ...running [compiled Go code][op-program]
* ...that runs an EVM

## Overview
* [`cannon-mipsevm`](./crates/mipsevm) - Contains the native implementation of the MIPS thread context emulator.
* [`preimage-oracle`](./crates/preimage) - Rust bindings for interacting as client or sever over the Pre-image Oracle ABI.
* [`cannon-contracts`](https://github.com/ethereum-optimism/optimism/tree/develop/packages/contracts-bedrock/src/cannon) - [*in OP monorepo*] Contains the Solidity implementation of the MIPS thread context and the Preimage Oracle.

## Credits

This repository is heavily inspired by the original [Cannon][cannon], built by [George Hotz][geohot] and members of the [OP Labs][op-labs] team. The original implementation is written in Go, and can be found [in the Optimism monorepo][cannon]. All
credits for the original idea and reference implementation of this concept go to these folks.

## Benchmarks

### `cannon-mipsevm` benchmarks

The below benchmark was ran on a 2021 Macbook Pro with an M1 Max and 32 GB of unified memory
on commit [`71b68d5`](https://github.com/anton-rs/cannon-rs/pull/17/commits/71b68d52fb858cfc544c1430b482aeaef460552e).

| Benchmark Name             | `cannon` mean (Reference) | `cannon-rs` mean    |
|----------------------------|---------------------------|---------------------|
| Memory Merkle Root (25MB)  | 736.94 ms                 | 29.58 µs (-99%)     |
| Memory Merkle Root (50MB)  | 1.54s                     | 7.25 ms (-99%)      |
| Memory Merkle Root (100MB) | 3.34s                     | 273.76 ms (-91.8%)  |
| Memory Merkle Root (200MB) | 6.30s                     | 1.65s (-73.81%)     |

*todo - execution benchmarks*

## Contributing

To get started, a few dependencies are required:
* [Rust toolchain][rustup]
    * Recommended: [`cargo-nextest`][nextest]
* [Go toolchain][golang]
* [binutils][binutils] 
    * **Note for macOS users:** Please avoid installing `binutils` using `brew`.

### Testing

```sh
# With `cargo-nextest`
cargo +nightly nextest run --release --all --all-features
# Without `cargo-nextest`
cargo +nightly t --release --all --all-features
```

### Linting and Formatting

```sh
cargo +nightly fmt --all -- && cargo +nightly clippy --all --all-features -- -D warnings
```

### Running Benchmarks

```sh
cargo +nightly bench --all --all-features
```

## Documentation

Rustdocs are available by running `cargo doc --open` after cloning the repo.

### Specification

The specification for both Cannon and the preimage oracle can be found in the [Optimism monorepo][monorepo].
* [Cannon specification][cannon-specs]
* [Preimage oracle specification][fpp-specs]

## Docker

The docker image for `cannon-rs` is located in the [docker](./docker) directory, and can be built using the
script provided.

[geohot]: https://github.com/geohot
[op-labs]: https://oplabs.co
[monorepo]: https://github.com/ethereum-optimism/optimism
[cannon]: https://github.com/ethereum-optimism/optimism/tree/develop/cannon
[op-program]: https://github.com/ethereum-optimism/optimism/tree/develop/op-program
[op-challenger]: https://github.com/ethereum-optimism/optimism/tree/develop/op-challenger
[rustup]: https://rustup.rs/
[golang]: https://go.dev/doc/install
[binutils]: https://www.gnu.org/software/binutils/
[nextest]: https://nexte.st/
[fpp-specs]: https://github.com/ethereum-optimism/optimism/blob/develop/specs/fault-proof.md
[cannon-specs]: https://github.com/ethereum-optimism/optimism/blob/develop/specs/cannon-fault-proof-vm.md
