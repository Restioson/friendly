#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![feature(
    const_generics,
    const_evaluatable_checked,
    const_fn,
)]
#![cfg_attr(
    feature = "const_init",
    feature(
        const_mut_refs,
        const_fn_fn_ptr_basics,
        const_panic,
        const_eval_limit,
    )
)]
#![cfg_attr(feature = "const_init", const_eval_limit = "10000000")]
#![allow(incomplete_features)]

#[cfg(feature = "const_init")]
mod const_init;

use core::borrow::BorrowMut;
use core::cmp;

/// A block in the bitmap
#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct Block {
    /// The order of the biggest block under this block - 1. 0 denotes used
    pub order_free: u8,
}

impl Block {
    pub const fn new_free(order: u8) -> Self {
        Block {
            order_free: order + 1,
        }
    }

    pub const fn new_used() -> Self {
        Block {
            order_free: 0,
        }
    }
}

#[inline]
pub const fn blocks_in_tree(levels: u8) -> usize {
    ((1 << levels) - 1) as usize
}

#[inline]
const fn blocks_in_level(level: u8) -> usize {
    blocks_in_tree(level + 1) - blocks_in_tree(level)
}

/// Flat tree things.
///
/// # Note
/// **1 INDEXED!**
mod flat_tree {
    #[inline]
    pub const fn left_child(index: usize) -> usize {
        index << 1
    }

    #[inline]
    pub const fn parent(index: usize) -> usize {
        index >> 1
    }
}

// const MAX_ORDER_SIZE: u8 = $BASE_ORDER + MAX_ORDER;
/// A tree of blocks. Contains the flat representation of the tree as a flat array
pub struct Tree<B, const LEVELS: u8, const BASE_ORDER: u8>
    where B: BorrowMut<[Block; blocks_in_tree(LEVELS)]>,
{
    /// Flat array representation of tree. Used with the help of the `flat_tree` module.
    flat_blocks: B,
}

impl<B, const LEVELS: u8, const BASE_ORDER: u8> Tree<B, LEVELS, BASE_ORDER>
    where B: BorrowMut<[Block; blocks_in_tree(LEVELS)]>,
{
    pub const fn max_order() -> u8 {
        LEVELS - 1
    }

    pub const fn max_order_size() -> u8 {
        Self::max_order() + BASE_ORDER
    }

    pub const fn total_blocks() -> usize {
        blocks_in_tree(LEVELS)
    }

    pub fn new<I>(usable: I, flat_blocks: B) -> Self
         where I: Iterator<Item=::core::ops::Range<usize>> + Clone,
    {
        let mut tree = Tree { flat_blocks };

        // Set blocks at order 0 (level = MAX_ORDER) in the holes to used & set
        // their parents accordingly. This is implemented by checking if the block falls
        // completely within a usable memory area.
        let mut block_begin: usize = 0;

        for block_index in (1 << Self::max_order())..(1 << (Self::max_order() + 1)) {
            let block_end = block_begin + (1 << BASE_ORDER) - 1;

            if !(usable.clone())
                .any(|area| (area.contains(&block_begin) && area.contains(&block_end)))
            {
                *tree.block_mut(block_index - 1) = Block::new_used();
            } else {
                *tree.block_mut(block_index - 1) = Block::new_free(0);
            }

            block_begin += 1 << BASE_ORDER;
        }

        let mut start: usize = 1 << (Self::max_order() - 1);
        for order in 1..=Self::max_order() {
            for node_index in start..(start + blocks_in_level(Self::max_order() - order)) {
                tree.update_block(node_index, order);
            }

            start >>= 1;
        }

        tree
    }

    #[inline]
    fn block_mut(&mut self, index: usize) -> &mut Block {
        &mut self.flat_blocks.borrow_mut()[index]
    }

    #[inline]
    pub fn block(&self, index: usize) -> &Block {
        &self.flat_blocks.borrow()[index]
    }

    /// Allocate a block of `desired_order` if one is available, returning a pointer
    /// relative to the tree (i.e `0` is the beginning of the tree's memory).
    pub fn allocate(&mut self, desired_order: u8) -> Option<usize> {
        assert!(desired_order <= Self::max_order(), "Block order > maximum order!");

        let root = self.block_mut(0);

        // If the root node has no orders free, or if it does not have at least the desired
        // order free, then no blocks are available
        if root.order_free == 0 || (root.order_free - 1) < desired_order {
            return None;
        }

        let mut addr: usize = 0;
        let mut node_index = 1;

        let max_level =  Self::max_order() - desired_order;

        for level in 0..max_level {
            let left_child_index = flat_tree::left_child(
                node_index
            );
            let left_child = self.block(left_child_index - 1);

            let o = left_child.order_free;
            // If the child is not used (o!=0) or (desired_order in o-1)
            // Due to the +1 offset, we need to subtract 1 from 0.
            // However, (o - 1) >= desired_order can be simplified to o > desired_order
            node_index = if o != 0 && o > desired_order {
                left_child_index
            } else {
                // Move over to the right: if the parent had a free order and the left didn't,
                // the right must, or the parent is invalid and does not uphold invariants.

                // Since the address is moving from the left hand side, we need to increase
                // it by the size, which is 2^(BASE_ORDER + order) bytes

                // We also only want to allocate on the order of the child, hence
                // subtracting 1
                addr += 1 << (( Self::max_order_size() - level - 1) as u32);
                left_child_index + 1
            };
        }

        let block = self.block_mut(node_index - 1);
        block.order_free = 0;

        self.update_blocks_above(node_index, desired_order);

        Some(addr)
    }

    /// Deallocate a block of memory from a pointer relative to the tree (e.g `0` is the
    /// beginning of the tree's memory) and the order of the block.
    #[inline]
    pub fn deallocate(&mut self, ptr: usize, order: u8) {
        assert!(order <= Self::max_order(), "Block order > maximum order!");

        let level = Self::max_order() - order;
        let level_offset = blocks_in_tree(level);
        let index = level_offset + ((ptr as usize) >> (order + BASE_ORDER)) + 1;

        assert!(index < Self::total_blocks(), "Block index {} out of bounds!", index);

        assert_eq!(
            self.block(index - 1).order_free,
            0,
            "Block to free (index {}) must be used!",
            index,
        );

        // Set to free
        self.block_mut(index - 1).order_free = order + 1;

        self.update_blocks_above(index, order);
    }

    /// Update a block from its children
    #[inline]
    fn update_block(&mut self, node_index: usize, order: u8) {
        assert_ne!(order, 0, "Order 0 does not have children and thus cannot be updated from them!");
        assert_ne!(node_index, 0, "Node index 0 is invalid in 1 index tree!");

        // The ZERO indexed left child index
        let left_index = flat_tree::left_child(node_index) - 1;

        let left = self.block(left_index).order_free;
        let right = self.block(left_index + 1).order_free;

        if (left == order) && (right == order) {
            // Merge blocks
            self.block_mut(node_index - 1).order_free = order + 1;
        } else {
            self.block_mut(node_index - 1).order_free = cmp::max(left, right);
        }
    }

    #[inline]
    fn update_blocks_above(&mut self, index: usize, order: u8) {
        let mut node_index = index;

        // Iterate upwards and set parents accordingly
        for order in order + 1..=Self::max_order() {
            node_index = flat_tree::parent(node_index);
            self.update_block(node_index, order);
        }
    }
}

#[cfg(test)]
mod test {
    use std::iter;
    use std::collections::BTreeSet;
    use super::*;
    use std::convert::TryInto;

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
        let mut tree: TestTree = Tree::new(
            [
                0x100000..0x385df8,
                0x386241..0x387000,
                0x786ff9..0x7fe0000usize,
            ].iter().map(|x| x.clone()),
            new_flat_blocks()
        );
        assert_eq!(tree.allocate(0), Some(0x100000));
    }

    #[test]
    fn test_flat_tree_fns() {
        use super::flat_tree::*;
        //    1
        //  2   3
        // 4 5 6 7
        assert_eq!(left_child(1), 2);
        assert_eq!(parent(2), 1);
    }

    #[test]
    fn test_blocks_in_tree() {
        assert_eq!(blocks_in_tree(3), 1 + 2 + 4);
        assert_eq!(blocks_in_tree(1), 1);
    }

    #[test]
    fn test_blocks_in_level() {
        assert_eq!(blocks_in_level(2), 4);
        assert_eq!(blocks_in_level(0), 1);
    }

    #[test]
    fn test_tree_runs_out_of_blocks() {
        let mut tree: TestTree = Tree::new(
            iter::once(0..(1 << 30 + 1)),
            new_flat_blocks()
        );
        let max_blocks = blocks_in_level(MAX_ORDER);

        for _ in 0..max_blocks {
            assert_ne!(tree.allocate(0), None);
        }
        assert_eq!(tree.allocate(0), None);
    }

    #[test]
    fn test_init_tree() {
        let tree: TestTree  = Tree::new(
            iter::once(0..(1 << 30 + 1)),
            new_flat_blocks()
        );

        // Highest level has 1 block, next has 2, next 4
        assert_eq!(tree.flat_blocks[0].order_free, 19);

        assert_eq!(tree.flat_blocks[1].order_free, 18);
        assert_eq!(tree.flat_blocks[2].order_free, 18);

        assert_eq!(tree.flat_blocks[3].order_free, 17);
        assert_eq!(tree.flat_blocks[4].order_free, 17);
        assert_eq!(tree.flat_blocks[5].order_free, 17);
        assert_eq!(tree.flat_blocks[6].order_free, 17);
    }

    #[test]
    fn test_allocate_exact() {
        let mut tree: TestTree  = Tree::new(
            iter::once(0..(1 << 30 + 1)),
            new_flat_blocks()
        );
        tree.allocate(3).unwrap();

        tree = Tree::new(
            iter::once(0..(1 << 30 + 1)),
            new_flat_blocks()
        );
        assert_eq!(tree.allocate(MAX_ORDER - 1), Some(0x0));
        assert_eq!(
            tree.allocate(MAX_ORDER - 1),
            Some(2usize.pow(MAX_ORDER_SIZE as u32) / 2)
        );
        assert_eq!(tree.allocate(0), None);
        assert_eq!(tree.allocate(MAX_ORDER - 1), None);

        tree = Tree::new(
            iter::once(0..(1 << 30 + 1)),
            new_flat_blocks()
        );
        assert_eq!(tree.allocate(MAX_ORDER), Some(0x0));
        assert_eq!(tree.allocate(MAX_ORDER), None);
    }


    #[test]
    fn test_free() {
        let mut tree: TestTree  = Tree::new(
            iter::once(0..(1 << 30 + 1)),
            new_flat_blocks()
        );

        let ptr = tree.allocate(3).unwrap();
        tree.deallocate(ptr, 3);

        let ptr2 = tree.allocate(3).unwrap();
        assert_eq!(ptr2, ptr);
        tree.deallocate(ptr2, 3);

        let ptr = tree.allocate(1).unwrap();
        tree.deallocate(ptr, 1);

        let ptr = tree.allocate(0).unwrap();
        let ptr2 = tree.allocate(0).unwrap();

        tree.deallocate(ptr, 0);
        assert_eq!(tree.allocate(0).unwrap(), ptr);
        tree.deallocate(ptr, 0);
        tree.deallocate(ptr2, 0);

        assert_eq!(tree.allocate(5).unwrap(), 0x0);
    }

    #[test]
    fn test_alloc_unique_addresses() {
        let max_blocks = blocks_in_level(MAX_ORDER);
        let mut seen = BTreeSet::new();
        let mut tree: TestTree  = Tree::new(
            iter::once(0..(1 << 30 + 1)),
            new_flat_blocks()
        );

        for _ in 0..max_blocks {
            let addr = tree.allocate(0).unwrap();

            if seen.contains(&addr) {
                panic!("Allocator must return addresses never been allocated before!");
            } else {
                seen.insert(addr);
            }
        }
    }
}
