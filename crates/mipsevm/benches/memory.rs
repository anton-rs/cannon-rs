use cannon_mipsevm::Memory;
use criterion::{criterion_group, criterion_main, Criterion};
use rand::RngCore;

fn merkle_root(c: &mut Criterion) {
    c.bench_function("Merkle Root (memory size = 25 MB)", |b| {
        let mut memory = Memory::default();
        let mut data = vec![0u8; 25_000_000];
        rand::thread_rng().fill_bytes(&mut data[..]);
        memory
            .set_memory_range(0, &data[..])
            .expect("Should not error");
        b.iter(|| {
            memory.merkle_root().unwrap();
        });
    });

    c.bench_function("Merkle Root (memory size = 50 MB)", |b| {
        let mut memory = Memory::default();
        let mut data = vec![0u8; 50_000_000];
        rand::thread_rng().fill_bytes(&mut data[..]);
        memory
            .set_memory_range(0, &data[..])
            .expect("Should not error");
        b.iter(|| {
            memory.merkle_root().unwrap();
        });
    });

    c.bench_function("Merkle Root (memory size = 100 MB)", |b| {
        let mut memory = Memory::default();
        let mut data = vec![0u8; 100_000_000];
        rand::thread_rng().fill_bytes(&mut data[..]);
        memory
            .set_memory_range(0, &data[..])
            .expect("Should not error");
        b.iter(|| {
            memory.merkle_root().unwrap();
        });
    });
}

criterion_group!(benches, merkle_root);
criterion_main!(benches);
