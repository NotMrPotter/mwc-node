// Copyright 2018 The Grin Developers
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

pub mod common;

use self::core::core::hash::Hashed;
use self::core::core::verifier_cache::LruVerifierCache;
use self::core::core::{Block, BlockHeader, Transaction};
use self::core::libtx;
use self::core::pow::Difficulty;
use self::keychain::{ExtKeychain, Keychain};
use self::util::RwLock;
use crate::common::*;
use grin_core as core;
use grin_keychain as keychain;
use grin_util as util;
use std::sync::Arc;

#[test]
fn test_transaction_pool_block_building() {
	util::init_test_logger();
	let keychain: ExtKeychain = Keychain::from_random_seed(false).unwrap();

	let db_root = ".mwc_block_building".to_string();
	clean_output_dir(db_root.clone());

	{
		let mut chain = ChainAdapter::init(db_root.clone()).unwrap();

		let verifier_cache = Arc::new(RwLock::new(LruVerifierCache::new()));

		// Initialize the chain/txhashset with an initial block
		// so we have a non-empty UTXO set.
		let add_block =
			|prev_header: BlockHeader, txs: Vec<Transaction>, chain: &mut ChainAdapter| {
				let height = prev_header.height + 1;
				let key_id = ExtKeychain::derive_key_id(1, height as u32, 0, 0, 0);
				let fee = txs.iter().map(|x| x.fee()).sum();
				let reward = libtx::reward::output(
					&keychain,
					&libtx::ProofBuilder::new(&keychain),
					&key_id,
					fee,
					false,
					height,
				)
				.unwrap();
				let mut block = Block::new(&prev_header, txs, Difficulty::min(), reward).unwrap();

				// Set the prev_root to the prev hash for testing purposes (no MMR to obtain a root from).
				block.header.prev_root = prev_header.hash();

				chain.update_db_for_block(&block);
				block
			};

		let block = add_block(BlockHeader::default(), vec![], &mut chain);
		let header = block.header;

		// Now create tx to spend that first coinbase (now matured).
		// Provides us with some useful outputs to test with.
		let initial_tx =
			test_transaction_spending_coinbase(&keychain, &header, vec![10, 20, 30, 40]);

		// Mine that initial tx so we can spend it with multiple txs
		let block = add_block(header, vec![initial_tx], &mut chain);
		let header = block.header;

		// Initialize a new pool with our chain adapter.
		let pool = RwLock::new(test_setup(Arc::new(chain.clone()), verifier_cache));

		let root_tx_1 = test_transaction(&keychain, vec![10, 20], vec![24]);
		let root_tx_2 = test_transaction(&keychain, vec![30], vec![28]);
		let root_tx_3 = test_transaction(&keychain, vec![40], vec![38]);

		let child_tx_1 = test_transaction(&keychain, vec![24], vec![22]);
		let child_tx_2 = test_transaction(&keychain, vec![38], vec![32]);

		{
			let mut write_pool = pool.write();

			// Add the three root txs to the pool.
			write_pool
				.add_to_pool(test_source(), root_tx_1.clone(), false, &header)
				.unwrap();
			write_pool
				.add_to_pool(test_source(), root_tx_2.clone(), false, &header)
				.unwrap();
			write_pool
				.add_to_pool(test_source(), root_tx_3.clone(), false, &header)
				.unwrap();

			// Now add the two child txs to the pool.
			write_pool
				.add_to_pool(test_source(), child_tx_1.clone(), false, &header)
				.unwrap();
			write_pool
				.add_to_pool(test_source(), child_tx_2.clone(), false, &header)
				.unwrap();

			assert_eq!(write_pool.total_size(), 5);
		}

		let txs = pool.read().prepare_mineable_transactions().unwrap();

		let block = add_block(header, txs, &mut chain);

		// Check the block contains what we expect.
		assert_eq!(block.inputs().len(), 4);
		assert_eq!(block.outputs().len(), 4);
		assert_eq!(block.kernels().len(), 6);

		assert!(block.kernels().contains(&root_tx_1.kernels()[0]));
		assert!(block.kernels().contains(&root_tx_2.kernels()[0]));
		assert!(block.kernels().contains(&root_tx_3.kernels()[0]));
		assert!(block.kernels().contains(&child_tx_1.kernels()[0]));
		assert!(block.kernels().contains(&child_tx_2.kernels()[0]));

		// Now reconcile the transaction pool with the new block
		// and check the resulting contents of the pool are what we expect.
		{
			let mut write_pool = pool.write();
			write_pool.reconcile_block(&block).unwrap();

			assert_eq!(write_pool.total_size(), 0);
		}
	}
	// Cleanup db directory
	clean_output_dir(db_root.clone());
}
