use super::message::Message;
use super::peer;
use super::server::Handle as ServerHandle;
use crate::blockchain::{self, Blockchain};
use crate::types::block::Block;
use crate::types::hash::{Hashable, H256};

use log::{debug, error, warn};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

#[cfg(any(test, test_utilities))]
use super::peer::TestReceiver as PeerTestReceiver;
#[cfg(any(test, test_utilities))]
use super::server::TestReceiver as ServerTestReceiver;
#[derive(Clone)]
pub struct Worker {
    msg_chan: smol::channel::Receiver<(Vec<u8>, peer::Handle)>,
    num_worker: usize,
    server: ServerHandle,
    blockchain: Arc<Mutex<Blockchain>>,
    orphan_buffer: Arc<Mutex<HashMap<H256, Block>>>,
}

impl Worker {
    pub fn new(
        num_worker: usize,
        msg_src: smol::channel::Receiver<(Vec<u8>, peer::Handle)>,
        server: &ServerHandle,
        blockchain: &Arc<Mutex<Blockchain>>,
        orphan_buffer: &Arc<Mutex<HashMap<H256, Block>>>,
    ) -> Self {
        Self {
            msg_chan: msg_src,
            num_worker,
            server: server.clone(),
            blockchain: Arc::clone(blockchain),
            orphan_buffer: Arc::clone(orphan_buffer),
        }
    }

    pub fn start(self) {
        let num_worker = self.num_worker;
        for i in 0..num_worker {
            let cloned = self.clone();
            thread::spawn(move || {
                cloned.worker_loop();
                warn!("Worker thread {} exited", i);
            });
        }
    }

    fn worker_loop(&self) {
        loop {
            let result = smol::block_on(self.msg_chan.recv());
            if let Err(e) = result {
                error!("network worker terminated {}", e);
                break;
            }
            let msg = result.unwrap();
            let (msg, mut peer) = msg;
            let msg: Message = bincode::deserialize(&msg).unwrap();
            let mut chain_unwrapped = self.blockchain.lock().unwrap();
            match msg {
                Message::Ping(nonce) => {
                    debug!("Ping: {}", nonce);
                    peer.write(Message::Pong(nonce.to_string()));
                }
                Message::Pong(nonce) => {
                    debug!("Pong: {}", nonce);
                }
                Message::NewBlockHashes(hashes) => {
                    let mut hashes_need_blocks = Vec::new();
                    for hash in hashes.clone() {
                        if !chain_unwrapped.block_map.contains_key(&hash) {
                            // if no block contains this hash, then we ask by sending GetBlocks
                            hashes_need_blocks.push(hash);
                        }
                    }
                    peer.write(Message::GetBlocks(hashes_need_blocks));
                }
                Message::GetBlocks(hashes) => {
                    let mut blocks_with_hashes = Vec::new();
                    for hash in hashes.clone() {
                        if chain_unwrapped.block_map.contains_key(&hash) {
                            let block = chain_unwrapped.block_map[&hash].clone();
                            blocks_with_hashes.push(block);
                        }
                    }
                    peer.write(Message::Blocks(blocks_with_hashes));
                }
                Message::Blocks(blocks) => {
                    let mut new_blocks = Vec::new();
                    let mut parent_blocks_missing = Vec::new();
                    for block in blocks.clone() {
                        let difficulty = block.get_difficulty();
                        let mut hash = block.hash();
                        // check if curr block hash contained in chain. If not, we insert it
                        if !chain_unwrapped.block_map.contains_key(&hash) {
                            // check if blocks parent is missing
                            let parent_block_hash = block.get_parent();
                            // let parent_block= chain_unwrapped.block_map[parent_block_hash];
                            let mut orphan_buffer_unwrapped = self.orphan_buffer.lock().unwrap();
                            if !chain_unwrapped.block_map.contains_key(&parent_block_hash) {
                                parent_blocks_missing.push(parent_block_hash);
                                orphan_buffer_unwrapped.insert(parent_block_hash, block.clone());
                            } else {
                                // do PoW checks
                                let parent_difficulty =
                                    chain_unwrapped.block_map[&parent_block_hash].get_difficulty();
                                if hash <= difficulty && difficulty == parent_difficulty {
                                    chain_unwrapped.insert(&block);
                                }

                                // check if block is a parent an orphan is waiting for
                                loop {
                                    if orphan_buffer_unwrapped.contains_key(&hash) {
                                        let orphan_block =
                                            orphan_buffer_unwrapped.remove(&hash).unwrap();
                                        chain_unwrapped.insert(&orphan_block);
                                        new_blocks.push(orphan_block.hash());
                                        self.server.broadcast(Message::NewBlockHashes(vec![
                                            orphan_block.hash(),
                                        ]));
                                        hash = orphan_block.hash();
                                    } else {
                                        break;
                                    }
                                }
                            }

                            new_blocks.push(hash);
                        }
                    }
                    if !parent_blocks_missing.is_empty() {
                        peer.write(Message::GetBlocks(parent_blocks_missing));
                    }
                    if !new_blocks.is_empty() {
                        self.server.broadcast(Message::NewBlockHashes(new_blocks));
                    }
                }
                _ => unimplemented!(),
            }
        }
    }
}

#[cfg(any(test, test_utilities))]
struct TestMsgSender {
    s: smol::channel::Sender<(Vec<u8>, peer::Handle)>,
}
#[cfg(any(test, test_utilities))]
impl TestMsgSender {
    fn new() -> (
        TestMsgSender,
        smol::channel::Receiver<(Vec<u8>, peer::Handle)>,
    ) {
        let (s, r) = smol::channel::unbounded();
        (TestMsgSender { s }, r)
    }

    fn send(&self, msg: Message) -> PeerTestReceiver {
        let bytes = bincode::serialize(&msg).unwrap();
        let (handle, r) = peer::Handle::test_handle();
        smol::block_on(self.s.send((bytes, handle))).unwrap();
        r
    }
}
#[cfg(any(test, test_utilities))]
/// returns two structs used by tests, and an ordered vector of hashes of all blocks in the blockchain
fn generate_test_worker_and_start() -> (TestMsgSender, ServerTestReceiver, Vec<H256>) {
    let (server, server_receiver) = ServerHandle::new_for_test();
    let (test_msg_sender, msg_chan) = TestMsgSender::new();
    let blockchain = Blockchain::new();
    let blockchain = Arc::new(Mutex::new(blockchain));
    let chain_unwrapped = blockchain.lock().unwrap();
    let orphan_buffer = Arc::new(Mutex::new(HashMap::new()));
    let worker = Worker::new(1, msg_chan, &server, &blockchain, &orphan_buffer);
    worker.start();
    (
        test_msg_sender,
        server_receiver,
        chain_unwrapped.all_blocks_in_longest_chain(),
    )
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod test {
    use crate::types::block::generate_random_block;
    use crate::types::hash::Hashable;
    use ntest::timeout;

    use super::super::message::Message;
    use super::generate_test_worker_and_start;

    #[test]
    #[timeout(60000)]
    fn reply_new_block_hashes() {
        let (test_msg_sender, _server_receiver, v) = generate_test_worker_and_start();
        let random_block = generate_random_block(v.last().unwrap());
        let mut peer_receiver =
            test_msg_sender.send(Message::NewBlockHashes(vec![random_block.hash()]));
        let reply = peer_receiver.recv();
        if let Message::GetBlocks(v) = reply {
            assert_eq!(v, vec![random_block.hash()]);
        } else {
            panic!();
        }
    }
    #[test]
    #[timeout(60000)]
    fn reply_get_blocks() {
        let (test_msg_sender, _server_receiver, v) = generate_test_worker_and_start();
        let h = v.last().unwrap().clone();
        let mut peer_receiver = test_msg_sender.send(Message::GetBlocks(vec![h.clone()]));
        let reply = peer_receiver.recv();
        if let Message::Blocks(v) = reply {
            assert_eq!(1, v.len());
            assert_eq!(h, v[0].hash())
        } else {
            panic!();
        }
    }
    #[test]
    #[timeout(60000)]
    fn reply_blocks() {
        let (test_msg_sender, server_receiver, v) = generate_test_worker_and_start();
        let random_block = generate_random_block(v.last().unwrap());
        let mut _peer_receiver = test_msg_sender.send(Message::Blocks(vec![random_block.clone()]));
        let reply = server_receiver.recv().unwrap();
        if let Message::NewBlockHashes(v) = reply {
            assert_eq!(v, vec![random_block.hash()]);
        } else {
            panic!();
        }
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST
