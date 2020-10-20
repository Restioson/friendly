use criterion::{Criterion, BenchmarkId, Throughput, criterion_group, criterion_main, black_box, BatchSize};
use friendly::tree::{Block, blocks_in_tree, Tree};
use std::iter;
use std::convert::TryInto;
use std::time::Duration;

pub const MAX_ORDER: u8 = 18;
const LEVELS: u8 = 19;
pub const BASE_ORDER: u8 = 12;
pub type GibTree = Tree<Box<[Block; blocks_in_tree(LEVELS)]>, LEVELS, BASE_ORDER>;

pub fn new() -> GibTree {
    let v: Vec<Block> = iter::repeat(Block::new_used()).take(blocks_in_tree(LEVELS)).collect();
    let blocks = v.into_boxed_slice().try_into().map_err(|_| panic!()).unwrap();
    GibTree::new_free(blocks)
}

fn throughput_frame(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocate frame");
    group.measurement_time(Duration::from_secs(6));

    for size in [0, 1, 2].iter() {
        let bytes = 1 << (BASE_ORDER as u64 + size);
        group.throughput(Throughput::Bytes(bytes));

        group.bench_function(
            BenchmarkId::new("allocate_order", format!("{}KiB blocks", bytes / 1024)),
           |b| {
               b.iter_batched_ref(
                   || new(),
                   |t| black_box(t.alloc_order(*size as u8)),
                   BatchSize::NumIterations(1 << (MAX_ORDER as u64 - size)),
               );
           }
        );
    }

    group.finish();
}

criterion_group!(benches, throughput_frame);
criterion_main!(benches);
