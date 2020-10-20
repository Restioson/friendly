use criterion::{Criterion, BenchmarkId, Throughput, criterion_group, criterion_main, black_box, BatchSize};
use friendly::tree::{Block, blocks_in_tree, Tree};
use std::iter;
use std::convert::TryInto;
use std::time::Duration;

const LEVELS: u8 = 25;
pub const MAX_ORDER: u8 = 24;
pub const BASE_ORDER: u8 = 6;
pub type HeapTree = Tree<Box<[Block; blocks_in_tree(LEVELS)]>, LEVELS, BASE_ORDER>;

pub fn new() -> HeapTree {
    let v: Vec<Block> = iter::repeat(Block::new_used()).take(blocks_in_tree(LEVELS)).collect();
    let blocks = v.into_boxed_slice().try_into().map_err(|_| panic!()).unwrap();
    HeapTree::new_free(blocks)
}

fn heap(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocate heap");
    group.measurement_time(Duration::from_secs(10));

    for size in [0, 1, 2, 3, 4, 5, 6, 7, 8].iter() {
        let bytes = 1 << (BASE_ORDER as u64 + size);
        group.throughput(Throughput::Elements(1));

        group.bench_function(
            BenchmarkId::new("allocate_order", format!("{} byte blocks", bytes)),
            |b| {
                b.iter_batched_ref(
                    || new(),
                    |t| black_box(t.alloc_order(*size as u8)),
                    BatchSize::NumIterations(1 << (MAX_ORDER as u64 - size)),
                );
            }
        );
    }
}

criterion_group!(benches, heap);
criterion_main!(benches);
