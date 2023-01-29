use criterion::{criterion_group, criterion_main, Criterion};
use tokio::runtime::Builder;

use kickoff::selection::*;

fn bench_build(c: &mut Criterion) {
    c.bench_function("build_path", |b| {
        b.to_async(Builder::new_multi_thread().enable_all().build().unwrap())
            .iter(|| async {
                let mut element_build = ElementListBuilder::new();
                element_build.add_path();
                element_build.build().await.unwrap();
            })
    });
}

criterion_group!(benches, bench_build);
criterion_main!(benches);
