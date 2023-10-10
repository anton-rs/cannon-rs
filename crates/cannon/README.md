# `cannon`

The cannon crate provides a high-level interface to run the Cannon kernel, which consists of the [MIPS32 emulator][mipsevm]
as well as the [preimage oracle server][preimage-oracle].

The interaction between these two processes is fully synchronous. While the emulator is running, the preimage oracle
server is blocked on waiting for hints and preimage requests from the emulator. During the time that the preimage oracle server
is working, the emulator is blocked on waiting for the preimage oracle server to respond.

```text
┌───────┐   ┌───────────────┐
│mipsevm│   │preimage-server│
└───┬───┘   └───────┬───────┘
    │               │        
    │     Hint      │        
    │──────────────>│        
    │               │        
    │   Ack hint    │        
    │<──────────────│        
    │               │        
    │ Get Preimage  │        
    │──────────────>│        
    │               │        
    │Return preimage│        
    │<──────────────│        
┌───┴───┐   ┌───────┴───────┐
│mipsevm│   │preimage-server│
└───────┘   └───────────────┘
```

[mipsevm]: ../mipsevm
[preimage-oracle]: ../preimage-oracle
