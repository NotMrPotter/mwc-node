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

use self::chain::types::NoopAdapter;
use self::chain::Chain;
use self::core::core::verifier_cache::LruVerifierCache;
use self::core::core::{Block, BlockHeader, Transaction};
use self::core::global::{self, ChainTypes};
use self::core::libtx;
use self::core::pow::{self, Difficulty};
use self::core::{consensus, genesis};
use self::keychain::{ExtKeychain, ExtKeychainPath, Keychain};
use self::util::RwLock;
use chrono::Duration;
use grin_chain as chain;
use grin_core as core;
use grin_keychain as keychain;
use grin_util as util;
use std::fs;
use std::sync::Arc;

fn clean_output_dir(dir_name: &str) {
	let _ = fs::remove_dir_all(dir_name);
}

fn setup(dir_name: &str) -> Chain {
	util::init_test_logger();
	clean_output_dir(dir_name);
	global::set_mining_mode(ChainTypes::AutomatedTesting);
	let genesis_block = pow::mine_genesis_block().unwrap();
	let verifier_cache = Arc::new(RwLock::new(LruVerifierCache::new()));
	chain::Chain::init(
		dir_name.to_string(),
		Arc::new(NoopAdapter {}),
		genesis_block,
		pow::verify_size,
		verifier_cache,
		false,
	)
	.unwrap()
}

fn reload_chain(dir_name: &str) -> Chain {
	let verifier_cache = Arc::new(RwLock::new(LruVerifierCache::new()));
	chain::Chain::init(
		dir_name.to_string(),
		Arc::new(NoopAdapter {}),
		genesis::genesis_dev(),
		pow::verify_size,
		verifier_cache,
		false,
	)
	.unwrap()
}

#[test]
fn data_files() {
	let chain_dir = ".mwc_df";
	//new block so chain references should be freed
	{
		let chain = setup(chain_dir);
		let keychain = ExtKeychain::from_random_seed(false).unwrap();

		for n in 1..4 {
			let prev = chain.head_header().unwrap();
			let next_header_info = consensus::next_difficulty(1, chain.difficulty_iter().unwrap());
			let pk = ExtKeychainPath::new(1, n as u32, 0, 0, 0).to_identifier();
			let reward = libtx::reward::output(
				&keychain,
				&libtx::ProofBuilder::new(&keychain),
				&pk,
				0,
				false,
				prev.height + 1,
			)
			.unwrap();
			let mut b =
				core::core::Block::new(&prev, vec![], next_header_info.clone().difficulty, reward)
					.unwrap();
			b.header.timestamp = prev.timestamp + Duration::seconds(60);
			b.header.pow.secondary_scaling = next_header_info.secondary_scaling;

			chain.set_txhashset_roots(&mut b).unwrap();

			pow::pow_size(
				&mut b.header,
				next_header_info.difficulty,
				global::proofsize(),
				global::min_edge_bits(),
			)
			.unwrap();

			chain
				.process_block(b.clone(), chain::Options::MINE)
				.unwrap();

			chain.validate(false).unwrap();
		}
	}
	// Now reload the chain, should have valid indices
	{
		let chain = reload_chain(chain_dir);
		chain.validate(false).unwrap();
	}
	// Cleanup chain directory
	clean_output_dir(chain_dir);
}

fn _prepare_block(kc: &ExtKeychain, prev: &BlockHeader, chain: &Chain, diff: u64) -> Block {
	let mut b = _prepare_block_nosum(kc, prev, diff, vec![]);
	chain.set_txhashset_roots(&mut b).unwrap();
	b
}

fn _prepare_block_tx(
	kc: &ExtKeychain,
	prev: &BlockHeader,
	chain: &Chain,
	diff: u64,
	txs: Vec<&Transaction>,
) -> Block {
	let mut b = _prepare_block_nosum(kc, prev, diff, txs);
	chain.set_txhashset_roots(&mut b).unwrap();
	b
}

fn _prepare_fork_block(kc: &ExtKeychain, prev: &BlockHeader, chain: &Chain, diff: u64) -> Block {
	let mut b = _prepare_block_nosum(kc, prev, diff, vec![]);
	chain.set_txhashset_roots_forked(&mut b, prev).unwrap();
	b
}

fn _prepare_fork_block_tx(
	kc: &ExtKeychain,
	prev: &BlockHeader,
	chain: &Chain,
	diff: u64,
	txs: Vec<&Transaction>,
) -> Block {
	let mut b = _prepare_block_nosum(kc, prev, diff, txs);
	chain.set_txhashset_roots_forked(&mut b, prev).unwrap();
	b
}

fn _prepare_block_nosum(
	kc: &ExtKeychain,
	prev: &BlockHeader,
	diff: u64,
	txs: Vec<&Transaction>,
) -> Block {
	let key_id = ExtKeychainPath::new(1, diff as u32, 0, 0, 0).to_identifier();

	let fees = txs.iter().map(|tx| tx.fee()).sum();
	let reward =
		libtx::reward::output(kc, &libtx::ProofBuilder::new(kc), &key_id, fees, false, prev.height + 1).unwrap();
	let mut b = match core::core::Block::new(
		prev,
		txs.into_iter().cloned().collect(),
		Difficulty::from_num(diff),
		reward,
	) {
		Err(e) => panic!("{:?}", e),
		Ok(b) => b,
	};
	b.header.timestamp = prev.timestamp + Duration::seconds(60);
	b.header.pow.total_difficulty = Difficulty::from_num(diff);
	b
}
