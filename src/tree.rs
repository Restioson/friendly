#[cfg(feature = "const_init")]
mod const_init;

use core::borrow::BorrowMut;
use core::cmp;
use core::alloc::Layout;
use core::ops::RangeInclusive;

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

const fn blocks_in_level(level: u8) -> usize {
    blocks_in_tree(level + 1) - blocks_in_tree(level)
}

#[test]
fn a() {
    panic!("{}", blocks_in_tree(6));
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

const fn log2_ceil(val: usize) -> u8 {
    let log2 = log2_floor(val);
    if val != (1usize << log2) {
        log2 + 1
    } else {
        log2
    }
}

const fn log2_floor(mut val: usize) -> u8 {
    let mut log2 = 0;
    while val > 1 {
        val >>= 1;
        log2 += 1;
    }
    log2 as u8
}

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

    pub const fn blocks_in_order(order: u8) -> usize {
        blocks_in_level(Self::max_order() - order)
    }

    pub fn new_free(flat_blocks: B) -> Self {
        let last_addr = Self::blocks_in_order(0) * (1 << BASE_ORDER) - 1;
        Self::new(Some(0..=last_addr), flat_blocks)
    }

    pub fn new<I>(usable: I, flat_blocks: B) -> Self
        where I: IntoIterator<Item = RangeInclusive<usize>> + Clone,
    {
        let mut tree = Tree { flat_blocks };

        // Set blocks at order 0 (level = MAX_ORDER) in the holes to used & set
        // their parents accordingly. This is implemented by checking if the block falls
        // completely within a usable memory area.
        let mut block_begin: usize = 0;

        for block_index in (1 << Self::max_order())..(1 << (Self::max_order() + 1)) {
            let block_end = block_begin + (1 << BASE_ORDER) - 1;

            if !(usable.clone().into_iter())
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
            for node_index in start..(start + Self::blocks_in_order(order)) {
                tree.update_block(node_index, order);
            }

            start >>= 1;
        }

        tree
    }

    /// Find the block order for a given layout
    fn order(layout: Layout) -> u8 {
        let layout = layout.pad_to_align();
        let log2 = log2_ceil(layout.size()) + 1;

        if log2 > BASE_ORDER {
            log2 - BASE_ORDER
        } else {
            0
        }
    }

    #[inline]
    fn block_mut(&mut self, index: usize) -> &mut Block {
        &mut self.flat_blocks.borrow_mut()[index]
    }

    #[inline]
    pub fn block(&self, index: usize) -> &Block {
        &self.flat_blocks.borrow()[index]
    }

    pub fn alloc_layout(&mut self, layout: Layout) -> Option<usize> {
        if layout.size() == 0 {
            return Some(layout.align()) // This is ok since it is a ZST
        }

        let order = Self::order(layout);
        if order > Self::max_order() {
            return None; // Cannot allocate greater than size of entire tree
        }

        self.alloc_order(order)
    }

    pub fn dealloc_layout(&mut self, ptr: usize, layout: Layout) {
        if layout.size() == 0 {
            return; // This is ok since it is a ZST
        }

        self.dealloc_order(ptr, Self::order(layout))
    }

    /// Allocate a block of `desired_order` if one is available, returning a pointer
    /// relative to the tree (i.e `0` is the beginning of the tree's memory).
    pub fn alloc_order(&mut self, desired_order: u8) -> Option<usize> {
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
    pub fn dealloc_order(&mut self, ptr: usize, order: u8) {
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
    use super::*;
    use std::convert::TryInto;

    type TestTree = Tree<Box<[Block; blocks_in_tree(19)]>, 19, 12>;

    fn new_flat_blocks() -> Box<[Block; blocks_in_tree(19)]> {
        let v: Vec<Block> = iter::repeat(Block::new_used()).take(blocks_in_tree(19)).collect();
        v.into_boxed_slice().try_into().map_err(|_| panic!()).unwrap()
    }

    #[test]
    fn test_init_tree() {
        let tree = TestTree::new(
            iter::once(0..=(1 << 30) - 1),
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
}
