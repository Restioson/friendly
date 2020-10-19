use criterion::{Criterion, BenchmarkId, Throughput, criterion_group, criterion_main, black_box, BatchSize};
use friendly::{Block, blocks_in_tree, Tree};
use std::iter;
use std::convert::TryInto;
use std::time::Duration;

mod gib_tree {
    use super::*;
    pub const MAX_ORDER: u8 = 18;
    const LEVELS: u8 = 19;
    pub const BASE_ORDER: u8 = 12;
    pub type GibTree = Tree<Box<[Block; blocks_in_tree(LEVELS)]>, LEVELS, BASE_ORDER>;

    pub fn new() -> GibTree {
        let v: Vec<Block> = iter::repeat(Block::new_used()).take(blocks_in_tree(LEVELS)).collect();
        let blocks = v.into_boxed_slice().try_into().map_err(|_| panic!()).unwrap();
        GibTree::new(iter::once(0..(1 << 30 + 1)), blocks)
    }
}

mod heap_tree {
    use super::*;
    const LEVELS: u8 = 25;
    pub const MAX_ORDER: u8 = 24;
    pub const BASE_ORDER: u8 = 6;
    pub type HeapTree = Tree<Box<[Block; blocks_in_tree(LEVELS)]>, LEVELS, BASE_ORDER>;

    pub fn new() -> HeapTree {
        let v: Vec<Block> = iter::repeat(Block::new_used()).take(blocks_in_tree(LEVELS)).collect();
        let blocks = v.into_boxed_slice().try_into().map_err(|_| panic!()).unwrap();
        HeapTree::new(iter::once(0..(1 << 30 + 1)), blocks)
    }
}

fn throughput_frame(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocate frame");
    group.measurement_time(Duration::from_secs(6));

    for size in [0, 1, 2].iter() {
        let bytes = 1 << (gib_tree::BASE_ORDER as u64 + size);
        group.throughput(Throughput::Bytes(bytes));

        group.bench_function(
            BenchmarkId::new("allocate_order", format!("{}KiB blocks", bytes / 1024)),
           |b| {
               b.iter_batched_ref(
                   || gib_tree::new(),
                   |t| black_box(t.allocate(0)),
                   BatchSize::NumIterations(1 << (gib_tree::MAX_ORDER as u64 - size)),
               );
           }
        );
    }

    group.finish();
}

fn throughput_heap(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocate heap");
    group.measurement_time(Duration::from_secs(10));

    for size in [0, 1, 2, 3, 4, 5, 6, 7, 8].iter() {
        let bytes = 1 << (heap_tree::BASE_ORDER as u64 + size);
        group.throughput(Throughput::Bytes(bytes));

        group.bench_function(
            BenchmarkId::new("allocate_order", format!("{} byte blocks", bytes)),
            |b| {
                b.iter_batched_ref(
                    || heap_tree::new(),
                    |t| black_box(t.allocate(0)),
                    BatchSize::NumIterations(1 << (heap_tree::MAX_ORDER as u64 - size)),
                );
            }
        );
    }
}

criterion_group!(
    benches,
    throughput_frame,
    throughput_heap
);
criterion_main!(benches);
