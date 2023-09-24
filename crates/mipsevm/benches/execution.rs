use cannon_mipsevm::{
    load_elf, patch_go, patch_stack,
    test_utils::{ClaimTestOracle, StaticOracle},
    InstrumentedState, PreimageOracle,
};
use criterion::{criterion_group, criterion_main, Bencher, Criterion};
use std::io::BufWriter;

fn bench_exec(
    elf_bytes: &[u8],
    oracle: impl PreimageOracle,
    compute_witness: bool,
    b: &mut Bencher,
) {
    let mut state = load_elf(elf_bytes).unwrap();
    patch_go(elf_bytes, &state).unwrap();
    patch_stack(&mut state).unwrap();

    let out = BufWriter::new(Vec::default());
    let err = BufWriter::new(Vec::default());
    let mut ins = InstrumentedState::new(state, oracle, out, err);

    b.iter(|| loop {
        if ins.state.exited {
            break;
        }
        ins.step(compute_witness).unwrap();
    })
}

fn execution(c: &mut Criterion) {
    c.bench_function("[No Witness] Execution (hello.elf)", |b| {
        let elf_bytes = include_bytes!("../../../example/bin/hello.elf");
        bench_exec(elf_bytes, StaticOracle::default(), false, b);
    });

    c.bench_function("[Witness] Execution (hello.elf)", |b| {
        let elf_bytes = include_bytes!("../../../example/bin/hello.elf");
        bench_exec(elf_bytes, StaticOracle::default(), true, b);
    });

    c.bench_function("[No Witness] Execution (claim.elf)", |b| {
        let elf_bytes = include_bytes!("../../../example/bin/claim.elf");
        bench_exec(elf_bytes, ClaimTestOracle::default(), false, b);
    });

    c.bench_function("[Witness] Execution (claim.elf)", |b| {
        let elf_bytes = include_bytes!("../../../example/bin/claim.elf");
        bench_exec(elf_bytes, ClaimTestOracle::default(), true, b);
    });
}

criterion_group!(benches, execution);
criterion_main!(benches);
