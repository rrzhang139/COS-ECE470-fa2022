use core::time;
use std::sync::{Arc, Mutex};
use std::{collections::HashMap, thread};

use crate::{network::message, types::address::Address};
use rand::Rng;
use ring::{
    digest,
    error::Unspecified,
    signature::{Ed25519KeyPair, EdDSAParameters, KeyPair, Signature, VerificationAlgorithm},
};
use serde::{Deserialize, Serialize};

use super::hash::{Hashable, H256};

use crate::network::server::Handle as NetworkServerHandle;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Mempool {
    pub tx_map: HashMap<H256, SignedTransaction>,
}

impl Mempool {
    pub fn new() -> Self {
        let tx_map = HashMap::new();
        Mempool { tx_map }
    }

    pub fn insert(&mut self, tx: &SignedTransaction) {
        // insert a tx in mempool
        let tx_hash = tx.hash();
        if !self.tx_map.contains_key(&tx_hash) {
            self.tx_map.insert(tx_hash.clone(), tx.clone());
        }
    }

    pub fn remove(&mut self, tx: &SignedTransaction) {
        // remove a tx from the mempool
        let tx_hash = tx.hash();
        if !self.tx_map.contains_key(&tx_hash) {
            self.tx_map.remove(&tx_hash);
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Transaction {
    pub sender: Address,
    pub receiver: Address,
    pub value: u8,
}

pub struct TransactionGenerator {}

impl TransactionGenerator {
    pub fn start(theta: u64, network: NetworkServerHandle, mempool: Arc<Mutex<Mempool>>) {
        use crate::types::key_pair;
        thread::spawn(move || loop {
            let mut rng = rand::thread_rng();
            let random_bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
            let random_bytes1: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
            let sender_addr = Address::from_public_key_bytes(&random_bytes);
            let receiver_addr = Address::from_public_key_bytes(&random_bytes1);
            let transaction = Transaction {
                sender: sender_addr,
                receiver: receiver_addr,
                value: 10,
            };
            let key = key_pair::random();
            let signature = sign(&transaction, &key).as_ref().to_vec();
            let public_key = key.public_key().as_ref().to_vec();
            let signed_tx = SignedTransaction {
                public_key,
                signature,
                transaction,
            };
            // add tx to mempool
            let mut unwrapped_mempool = mempool.lock().unwrap();
            unwrapped_mempool.insert(&signed_tx);
            drop(unwrapped_mempool);
            // broadcast the tx hash to network
            network.broadcast(message::Message::NewTransactionHashes(vec![
                signed_tx.hash()
            ]));
            if theta != 0 {
                let interval = time::Duration::from_micros(theta as u64);
                thread::sleep(interval);
            }
        });
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct SignedTransaction {
    pub transaction: Transaction,
    pub signature: Vec<u8>,
    pub public_key: Vec<u8>,
}

impl Hashable for SignedTransaction {
    fn hash(&self) -> H256 {
        // serialize SignedTransaction into bytes
        let serialize = bincode::serialize(&self).unwrap();
        let mut ctx = digest::Context::new(&digest::SHA256);
        ctx.update(&serialize);
        ctx.finish().into()
    }
}

/// Create digital signature of a transaction
pub fn sign(t: &Transaction, key: &Ed25519KeyPair) -> Signature {
    let sender = t.sender.0.as_ref();
    let receiver = t.receiver.0.as_ref();
    let tx_array = [&sender[..], &receiver[..], &[t.value]].concat();
    key.sign(&tx_array[..])
}

/// Verify digital signature of a transaction, using public key instead of secret key
pub fn verify(t: &Transaction, public_key: &[u8], signature: &[u8]) -> bool {
    // create tx message byte array
    let sender = t.sender.0.as_ref();
    let receiver = t.receiver.0.as_ref();
    let tx_array = [&sender[..], &receiver[..], &[t.value]].concat();

    let pk_vector: Vec<u8> = public_key.as_ref().to_vec();
    let signature_vector: Vec<u8> = signature.as_ref().to_vec();
    let a = EdDSAParameters {};
    let result = VerificationAlgorithm::verify(
        &a,
        untrusted::Input::from(&pk_vector[..]),
        untrusted::Input::from(&tx_array[..]),
        untrusted::Input::from(&signature_vector[..]),
    );
    match result {
        Ok(()) => true,
        Err(Unspecified) => false,
    }
}

#[cfg(any(test, test_utilities))]
pub fn generate_random_transaction() -> Transaction {
    let mut rng = rand::thread_rng();
    let random_bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    let random_bytes1: Vec<u8> = (0..32).map(|_| rng.gen()).collect();

    let sender_addr = Address::from_public_key_bytes(&random_bytes);
    let receiver_addr = Address::from_public_key_bytes(&random_bytes1);
    Transaction {
        sender: sender_addr,
        receiver: receiver_addr,
        value: 0,
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::key_pair;
    use ring::signature::KeyPair;

    #[test]
    fn sign_verify() {
        let t = generate_random_transaction();
        let key = key_pair::random();
        let signature = sign(&t, &key);
        assert!(verify(&t, key.public_key().as_ref(), signature.as_ref()));
    }
    #[test]
    fn sign_verify_two() {
        let t = generate_random_transaction();
        let key = key_pair::random();
        let signature = sign(&t, &key);
        let key_2 = key_pair::random();
        let t_2 = generate_random_transaction();
        assert!(!verify(&t_2, key.public_key().as_ref(), signature.as_ref()));
        assert!(!verify(&t, key_2.public_key().as_ref(), signature.as_ref()));
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST
