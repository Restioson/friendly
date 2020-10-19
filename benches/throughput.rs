use criterion::{Criterion, BenchmarkId, Throughput, criterion_group, criterion_main, black_box, BatchSize};
use friendly::{Block, blocks_in_tree, Tree};
use std::iter;
use std::convert::TryInto;
use std::time::Duration;

const MAX_ORDER: u8 = 18;
const LEVELS: u8 = 19;
const BASE_ORDER: u8 = 12;
type TestTree = Tree<Box<[Block; blocks_in_tree(LEVELS)]>, LEVELS, BASE_ORDER>;

fn new() -> TestTree {
    let v: Vec<Block> = iter::repeat(Block::new_used()).take(blocks_in_tree(19)).collect();
    let blocks = v.into_boxed_slice().try_into().map_err(|_| panic!()).unwrap();
    TestTree::new(iter::once(0..(1 << 30 + 1)), blocks)
}

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("Allocate");
    group.measurement_time(Duration::from_secs(6));

    for size in [0, 1, 2].iter() {
        group.throughput(Throughput::Bytes(1 << (BASE_ORDER as u64 + size)));

        group.bench_function(
            BenchmarkId::new("allocate_order", *size),
           |b| {
               b.iter_batched_ref(
                   || new(),
                   |t| black_box(t.allocate(0)),
                   BatchSize::NumIterations(1 << (MAX_ORDER as u64 - size)),
               );
           }
        );
    }

    group.finish();
}

criterion_group!(benches, bench_throughput);
criterion_main!(benches);
