use std::iter;
use std::collections::BTreeSet;
use std::convert::TryInto;
use friendly::tree::{Tree, Block, blocks_in_tree};

type TestTree = Tree<Box<[Block; blocks_in_tree(19)]>, 19, 12>;
const MAX_ORDER: u8 = 18;
const BASE_ORDER: u8 = 12;
const MAX_ORDER_SIZE: u8 = BASE_ORDER + MAX_ORDER;

fn new_flat_blocks() -> Box<[Block; blocks_in_tree(19)]> {
    let v: Vec<Block> = iter::repeat(Block::new_used()).take(blocks_in_tree(19)).collect();
    v.into_boxed_slice().try_into().map_err(|_| panic!()).unwrap()
}

#[test]
fn test_usable() {
    let mut tree = TestTree::new(
        [
            0x100000..=0x385df7,
            0x386241..=0x386999,
            0x786ff9..=0x7fd9999usize,
        ].iter().map(|x| x.clone()),
        new_flat_blocks()
    );
    // TODO more tests
    assert_eq!(tree.alloc_order(0), Some(0x100000));
}

#[test]
fn test_blocks_in_tree() {
    assert_eq!(blocks_in_tree(3), 1 + 2 + 4);
    assert_eq!(blocks_in_tree(1), 1);
}

#[test]
fn test_blocks_in_level() {
    assert_eq!(TestTree::blocks_in_order(TestTree::max_order() - 2), 4);
    assert_eq!(TestTree::blocks_in_order(TestTree::max_order()), 1);
}

#[test]
fn test_tree_runs_out_of_blocks() {
    let mut tree= TestTree::new_free(new_flat_blocks());
    let max_blocks = TestTree::blocks_in_order(0);

    for _ in 0..max_blocks {
        assert_ne!(tree.alloc_order(0), None);
    }
    assert_eq!(tree.alloc_order(0), None);
}

#[test]
fn test_allocate_exact() {
    let mut tree = TestTree::new_free(new_flat_blocks());
    tree.alloc_order(3).unwrap();

    tree = TestTree::new_free(new_flat_blocks());
    assert_eq!(tree.alloc_order(MAX_ORDER - 1), Some(0x0));
    assert_eq!(
        tree.alloc_order(MAX_ORDER - 1),
        Some(2usize.pow(MAX_ORDER_SIZE as u32) / 2)
    );
    assert_eq!(tree.alloc_order(0), None);
    assert_eq!(tree.alloc_order(MAX_ORDER - 1), None);

    tree = TestTree::new_free(new_flat_blocks());
    assert_eq!(tree.alloc_order(MAX_ORDER), Some(0x0));
    assert_eq!(tree.alloc_order(MAX_ORDER), None);
}


#[test]
fn test_free() {
    let mut tree = TestTree::new(
        Some(0..=(1 << 30) - 1),
        new_flat_blocks()
    );

    let ptr = tree.alloc_order(3).unwrap();
    tree.dealloc_order(ptr, 3);

    let ptr2 = tree.alloc_order(3).unwrap();
    assert_eq!(ptr2, ptr);
    tree.dealloc_order(ptr2, 3);

    let ptr = tree.alloc_order(1).unwrap();
    tree.dealloc_order(ptr, 1);

    let ptr = tree.alloc_order(0).unwrap();
    let ptr2 = tree.alloc_order(0).unwrap();

    tree.dealloc_order(ptr, 0);
    assert_eq!(tree.alloc_order(0).unwrap(), ptr);
    tree.dealloc_order(ptr, 0);
    tree.dealloc_order(ptr2, 0);

    assert_eq!(tree.alloc_order(5).unwrap(), 0x0);
}

#[test]
fn test_alloc_unique_addresses() {
    let max_blocks = TestTree::blocks_in_order(0);
    let mut seen = BTreeSet::new();
    let mut tree = TestTree::new_free(new_flat_blocks());

    for _ in 0..max_blocks {
        let addr = tree.alloc_order(0).unwrap();

        if seen.contains(&addr) {
            panic!("Allocator must return addresses never been allocated before!");
        } else {
            seen.insert(addr);
        }
    }
}

