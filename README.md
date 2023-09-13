<img align="right" width="150" height="150" top="100" src="./assets/logo.png">

# `boilerplate-rs` • [![ci](https://github.com/clabby/boilerplate-rs/actions/workflows/ci.yaml/badge.svg?label=ci)](https://github.com/clabby/boilerplate-rs/actions/workflows/ci.yaml) ![license](https://img.shields.io/badge/License-MIT-green.svg?label=license)

A dead simple boilerplate for Rust projects.

**Project Structure**
```
├── assets
│   └── logo.png
├── bin
│   ├── Cargo.toml
│   └── src
│       └── boilerplate.rs
├── Cargo.lock
├── Cargo.toml
├── crates
│   └── example
│       ├── Cargo.toml
│       └── src
│           └── lib.rs
├── LICENSE.md
└── README.md
```

**Pre-installed crates**
- [clap](https://crates.io/crates/clap)
- [tracing](https://crates.io/crates/tracing)
- [anyhow](https://crates.io/crates/anyhow)

**Getting Started**
1. Clone the repo
```
git clone git@github.com:clabby/boilerplate-rs.git
```
2. Run the binary
```
cargo r --bin boilerplate-rs
```
