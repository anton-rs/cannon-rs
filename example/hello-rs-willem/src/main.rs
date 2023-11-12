//! `cannon-rs` program template, provided by Willem Olding in the other [`Cannon-rs`](https://github.com/BadBoiLabs/Cannon-rs/tree/main).

#![no_std]
#![no_main]
#![feature(core_intrinsics)]
#![feature(alloc_error_handler)]
#![feature(asm_experimental_arch)]

extern crate alloc;

const HEAP_SIZE: usize = 0x400000;

use cannon_heap::init_heap;
use cannon_io::prelude::*;

#[no_mangle]
pub extern "C" fn _start() {
    init_heap!(HEAP_SIZE);

    print("hello world!\n").unwrap();

    exit(0);
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let msg = alloc::format!("panic: {}", info);
    let _ = print(&msg);
    exit(2);
}

#[alloc_error_handler]
fn alloc_error_handler(_layout: alloc::alloc::Layout) -> ! {
    let _ = print("alloc error!");
    exit(3);
}
