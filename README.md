<h1 align="center">
  <img src="./assets/banner.png" alt="Markdownify" height="300">
  <br>
  <code>cannon-rs</code>
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
</p>

<p align="center">
  <a href="#whats-a-cannon">What's a Cannon?</a> •
  <a href="#overview">Overview</a> •
  <a href="#credits">Credits</a> •
  <a href="#usage">Usage</a> •
  <a href="#contributing">Contributing</a> •
  <a href="#documentation">Documentation</a> •
  <a href="#docker">Docker</a>
</p>

## What's a Cannon?

Cannon is a single MIPS thread context emulator that runs on the EVM. It's used primarily to run the [op-program][op-program], or the fault proof program,
which is Go code modeling a stripped-down version of `op-geth`'s state transition code as well as the derivation pipeline, that is then compiled to MIPS.
Cannon also features a native implementation of the MIPS thread context that is identical to the on-chain implementation, and this library is used by the
[op-challenger][op-challenger] to generate state hashes while participating in the interactive dispute protocol.

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

## Credits

This repository is heavily inspired by the original [Cannon][cannon], built by [George Hotz][geohot] and members of the [OP Labs][op-labs] team. The original implementation is written in Go, and can be found [in the Optimism monorepo][cannon]. All
credits for the original idea and reference implementation of this concept go to these fine folks.

## Usage

*todo*

## Contributing

To get started, a few dependencies are required:
* [Rust toolchain][rustup]
    * Recommended: [`cargo-nextest`][nextest]
* [Go toolchain][golang]
* [binutils][binutils]

### Testing

```sh
# With `cargo-nextest`
cargo nextest run --all --all-features
# Without `cargo-nextest`
cargo t --all --all-features
```

### Linting and Formatting

```sh
cargo +nightly fmt -- && cargo +nightly clippy --all --all-features -- -D warnings
```

### Running Benchmarks
*todo*

## Documentation

Rustdocs are available by running `cargo doc --open` after cloning the repo.

## Docker

*todo*

[geohot]: https://github.com/geohot
[op-labs]: https://oplabs.co
[cannon]: https://github.com/ethereum-optimism/optimism/tree/develop/cannon
[op-program]: https://github.com/ethereum-optimism/optimism/tree/develop/op-program
[op-challenger]: https://github.com/ethereum-optimism/optimism/tree/develop/op-challenger
[rustup]: https://rustup.rs/
[golang]: https://go.dev/doc/install
[binutils]: https://www.gnu.org/software/binutils/
[nextest]: https://nexte.st/
