# [`Cannon-rs`][cannon-rs-willem] example program

This is an example Rust program for Cannon that uses Willem Olding's [program template][program-template-willem].

## Building

The program can be built using Badboilabs' provided container:

```sh
docker run \
    --rm \
    --platform linux/amd64 \
    -v `pwd`/:/code \
    -w="/code" \
    ghcr.io/badboilabs/cannon-rs/builder:main cargo build --release -Zbuild-std && \
    cp target/mips-unknown-none/release/hello-rs-willem ../bin/hello-willem-rs.elf
```

[cannon-rs-willem]: https://github.com/BadBoiLabs/Cannon-rs
[program-template-willem]: https://github.com/BadBoiLabs/Cannon-rs/tree/main/project-template
