<img align="right" width="150" height="150" top="100" src="./assets/logo.png">

# `cannon-rs` • [![ci](https://github.com/clabby/cannon-rs/actions/workflows/ci.yaml/badge.svg?label=ci)](https://github.com/clabby/cannon-rs/actions/workflows/ci.yaml) ![license](https://img.shields.io/badge/License-MIT-green.svg?label=license)

`cannon-rs` is an alternative implementation of [Cannon][cannon] in Rust.

* It's Rust code
* ...that was originally Go code
* ...that runs an EVM
* ...emulating a MIPS machine
* ...running compiled Go code
* ...that runs an EVM

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
[rustup]: https://rustup.rs/
[golang]: https://go.dev/doc/install
[binutils]: https://www.gnu.org/software/binutils/
[nextest]: https://nexte.st/
