use criterion::{Criterion, BenchmarkId, Throughput, criterion_group, criterion_main, black_box, BatchSize};
use friendly::tree::{Block, blocks_in_tree, Tree};
use std::iter;
use std::convert::TryInto;
use std::time::Duration;


const LEVELS: u8 = 25;
pub const BASE_ORDER: u8 = 4;
pub type SmallTree = Tree<Box<[Block; blocks_in_tree(LEVELS)]>, LEVELS, BASE_ORDER>;

pub fn new() -> SmallTree {
    let v: Vec<Block> = iter::repeat(Block::new_used()).take(blocks_in_tree(LEVELS)).collect();
    let blocks = v.into_boxed_slice().try_into().map_err(|_| panic!()).unwrap();
    SmallTree::new_free(blocks)
}

fn small(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocate small");
    group.measurement_time(Duration::from_secs(8));

    for size in [0, 1, 2, 3, 4, 5, 6, 7, 8].iter() {
        let bytes = 1 << (BASE_ORDER as u64 + *size as u64);
        let blocks = SmallTree::blocks_in_order(*size) as u64;
        group.throughput(Throughput::Elements(1));

        group.bench_function(
            BenchmarkId::new("allocate_order", format!("{} byte blocks", bytes)),
            |b| {
                b.iter_batched_ref(
                    || new(),
                    |t| assert!(black_box(t.alloc_order(*size)).is_some()),
                    BatchSize::NumIterations(blocks),
                );
            }
        );
    }
}
criterion_group!(benches, small);
criterion_main!(benches);
