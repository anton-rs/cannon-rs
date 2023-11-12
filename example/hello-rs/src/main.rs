use std::io::{self, Write};

fn main() {
    io::stdout().write_all(b"hello world!\n").unwrap();
}
