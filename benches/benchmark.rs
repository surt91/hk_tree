#[macro_use]
extern crate criterion;

use criterion::Criterion;

extern crate hk;
use hk::HegselmannKrause;

fn criterion_benchmark(c: &mut Criterion) {
    let mut hk = HegselmannKrause::new(1000, 0., 1., 13);
    c.bench_function("hk N=1000 sync sweep", |b| b.iter(|| hk.sweep_naive()));

    let mut hk = HegselmannKrause::new(1000, 0., 1., 13);
    c.bench_function("hk N=1000 sync btree sweep", |b| b.iter(|| hk.sweep_tree()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
