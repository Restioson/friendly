use super::*;

const fn const_max(left: u8, right: u8) -> u8 {
    if left > right {
        left
    } else {
        right
    }
}

impl<const LEVELS: u8, const BASE_ORDER: u8> Tree<[Block; blocks_in_tree(LEVELS)], LEVELS, BASE_ORDER> {
    pub const fn new_all_free() -> Self {
        let flat_blocks = [Block::new_free(0); blocks_in_tree(LEVELS)];
        let mut tree = Tree { flat_blocks };

        let mut start: usize = 1 << (Self::max_order() - 1);
        let mut order = 1;
        while order <= Self::max_order() {
            let mut node_index = start;
            let max = start + blocks_in_level(Self::max_order() - order);
            while node_index < max {
                tree.const_update_block(node_index, order);
                node_index += 1;
            }

            start >>= 1;
            order += 1;
        }

        tree
    }

    #[inline]
    const fn const_update_block(&mut self, node_index: usize, order: u8) {
        if order == 0 {
            panic!("Order 0 does not have children and thus cannot be updated from them!");
        }

        if node_index == 0 {
            panic!("Node index 0 is invalid in 1 index tree!");
        }

        // The ZERO indexed left child index
        let left_index = flat_tree::left_child(node_index) - 1;

        let left = self.flat_blocks[left_index].order_free;
        let right = self.flat_blocks[left_index + 1].order_free;

        if (left == order) && (right == order) {
            // Merge blocks
            self.flat_blocks[node_index - 1].order_free = order + 1;
        } else {
            self.flat_blocks[node_index - 1].order_free = const_max(left, right);
        }
    }
}

