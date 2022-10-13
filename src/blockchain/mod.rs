use std::collections::HashMap;
use std::error::Error;

use crate::types::block::{Block, Header};
use crate::types::hash::{Hashable, H256};
use crate::types::merkle::MerkleTree;

pub struct Blockchain {
    // hashmap to store blocks
    pub block_map: HashMap<H256, Block>,
    // hasmap from block hash to height
    pub block_heights: HashMap<H256, usize>,
    // latest block
    latest_block: H256,
}

impl Blockchain {
    /// Create a new blockchain, only containing the genesis block
    pub fn new() -> Self {
        let parent: H256 = [255u8; 32].into();
        let nonce = 0u32;
        let bytes = [255u8; 32];
        // bytes[2] = 1u8;
        let difficulty: H256 = bytes.into(); // remember the difficulty is the number of zeros on the left until it hits the first nonzero value
        let tx: Vec<H256> = Vec::new();
        let empty_tree = MerkleTree::new(&tx);
        let merkle_root = empty_tree.root();
        let genesis_block = Block {
            header: Header {
                parent,
                nonce,
                difficulty,
                timestamp: 0,
                merkle_root,
            },
            data: { Vec::new() },
        };
        let mut blocks = HashMap::new();
        let genesis_block_hash = genesis_block.hash();
        blocks.insert(genesis_block_hash, genesis_block);
        let mut block_heights = HashMap::new();
        block_heights.insert(genesis_block_hash, 0);

        Self {
            block_map: blocks,
            block_heights,
            latest_block: genesis_block_hash,
        }
    }

    /// Insert a block into blockchain
    pub fn insert(&mut self, block: &Block) {
        let parent = block.header.parent;
        let hash = block.hash();
        self.block_map.insert(hash, block.clone());
        let new_block_height = self.block_heights[&parent] + 1;
        self.block_heights.insert(hash, new_block_height);
        if new_block_height > self.block_heights[&self.latest_block] {
            self.latest_block = hash;
        }
    }

    /// Get the last block's hash of the longest chain
    pub fn tip(&self) -> H256 {
        self.latest_block
    }

    /// Get all blocks' hashes of the longest chain, ordered from genesis to the tip
    pub fn all_blocks_in_longest_chain(&self) -> Vec<H256> {
        let mut list = Vec::new();
        let genesis_parent: H256 = [255u8; 32].into();
        let mut curr_block_hash = self.latest_block;
        while genesis_parent != curr_block_hash {
            list.push(curr_block_hash.clone());
            curr_block_hash = self.block_map[&curr_block_hash].get_parent();
        }
        list.reverse();
        list
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::block::generate_random_block;
    use crate::types::hash::Hashable;

    #[test]
    fn insert_one() {
        let mut blockchain = Blockchain::new();
        let genesis_hash = blockchain.tip();
        let block = generate_random_block(&genesis_hash);
        blockchain.insert(&block);
        assert_eq!(blockchain.tip(), block.hash());
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST
