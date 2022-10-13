use crate::types::hash::{Hashable, H256};
use ring::digest;
use serde::{Deserialize, Serialize};

use super::transaction::SignedTransaction;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Header {
    pub(crate) parent: H256,
    pub(crate) nonce: u32,
    pub(crate) difficulty: H256,
    pub(crate) timestamp: u32,
    pub(crate) merkle_root: H256,
}

impl Hashable for Header {
    fn hash(&self) -> H256 {
        // serialize Header into bytes
        let serialize = bincode::serialize(&self).unwrap();
        let mut ctx = digest::Context::new(&digest::SHA256);
        ctx.update(&serialize);
        ctx.finish().into()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub(crate) header: Header,
    pub(crate) data: Vec<SignedTransaction>,
}

impl Hashable for Block {
    fn hash(&self) -> H256 {
        // hash header
        self.header.hash()
    }
}

impl Block {
    pub fn get_parent(&self) -> H256 {
        self.header.parent
    }

    pub fn get_difficulty(&self) -> H256 {
        self.header.difficulty
    }
}

#[cfg(any(test, test_utilities))]
pub fn generate_random_block(parent: &H256) -> Block {
    use crate::types::merkle::MerkleTree;

    let tx: Vec<H256> = Vec::new();
    let mut nonce: u32 = rand::random();
    let mut bytes = [u8::MAX; 32];
    bytes[0] = 0;
    bytes[1] = 0;
    let difficulty: H256 = bytes.into();
    let empty_tree = MerkleTree::new(&tx);
    let merkle_root = empty_tree.root();
    Block {
        header: Header {
            parent: *parent,
            nonce,
            difficulty,
            timestamp: 0,
            merkle_root: merkle_root,
        },
        data: { Vec::new() },
    }
}
