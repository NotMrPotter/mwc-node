// Copyright 2018 The Grin Developers
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

//! All the rules required for a cryptocurrency to have reach consensus across
//! the whole network are complex and hard to completely isolate. Some can be
//! simple parameters (like block reward), others complex algorithms (like
//! Merkle sum trees or reorg rules). However, as long as they're simple
//! enough, consensus-relevant constants and short functions should be kept
//! here.

use std::cmp::{max, min};

use crate::core::block::HeaderVersion;
use crate::global;
use crate::pow::Difficulty;

/// A grin is divisible to 10^9, following the SI prefixes
pub const GRIN_BASE: u64 = 1_000_000_000;
/// Milligrin, a thousand of a grin
pub const MILLI_GRIN: u64 = GRIN_BASE / 1_000;
/// Microgrin, a thousand of a milligrin
pub const MICRO_GRIN: u64 = MILLI_GRIN / 1_000;
/// Nanogrin, smallest unit, takes a billion to make a grin
pub const NANO_GRIN: u64 = 1;

/// Block interval, in seconds, the network will tune its next_target for. Note
/// that we may reduce this value in the future as we get more data on mining
/// with Cuckoo Cycle, networks improve and block propagation is optimized
/// (adjusting the reward accordingly).
pub const BLOCK_TIME_SEC: u64 = 60;

/// MWC - Here is a block reward.
/// The block subsidy amount, one grin per second on average
//pub const REWARD: u64 = BLOCK_TIME_SEC * GRIN_BASE;

/// Actual block reward for a given total fee amount
pub fn reward(fee: u64, height: u64) -> u64 {
	// MWC has block reward schedule similar to bitcoin
	let block_reward = calc_mwc_block_reward(height);
	block_reward.saturating_add(fee)
}

/// MWC  genesis block reward in nanocoins (10M coins)
pub const GENESIS_BLOCK_REWARD: u64 = 10_000_000_000_000_000 + 41_800_000;

/// Nominal height for standard time intervals, hour is 60 blocks
pub const HOUR_HEIGHT: u64 = 3600 / BLOCK_TIME_SEC;
/// A day is 1440 blocks
pub const DAY_HEIGHT: u64 = 24 * HOUR_HEIGHT;
/// A week is 10_080 blocks
pub const WEEK_HEIGHT: u64 = 7 * DAY_HEIGHT;
/// A year is 524_160 blocks
pub const YEAR_HEIGHT: u64 = 52 * WEEK_HEIGHT;

/// Number of blocks before a coinbase matures and can be spent
pub const COINBASE_MATURITY: u64 = DAY_HEIGHT;

/// Ratio the secondary proof of work should take over the primary, as a
/// function of block height (time). Starts at 90% losing a percent
/// approximately every week. Represented as an integer between 0 and 100.
pub fn secondary_pow_ratio(height: u64) -> u64 {
	90u64.saturating_sub(height / (2 * YEAR_HEIGHT / 90))
}

/// The AR scale damping factor to use. Dependent on block height
/// to account for pre HF behavior on testnet4.
fn ar_scale_damp_factor(_height: u64) -> u64 {
	AR_SCALE_DAMP_FACTOR
}

/// Cuckoo-cycle proof size (cycle length)
pub const PROOFSIZE: usize = 42;

/// Default Cuckatoo Cycle edge_bits, used for mining and validating.
pub const DEFAULT_MIN_EDGE_BITS: u8 = 31;

/// Cuckaroo proof-of-work edge_bits, meant to be ASIC resistant.
pub const SECOND_POW_EDGE_BITS: u8 = 29;

/// Original reference edge_bits to compute difficulty factors for higher
/// Cuckoo graph sizes, changing this would hard fork
pub const BASE_EDGE_BITS: u8 = 24;

/// Default number of blocks in the past when cross-block cut-through will start
/// happening. Needs to be long enough to not overlap with a long reorg.
/// Rational
/// behind the value is the longest bitcoin fork was about 30 blocks, so 5h. We
/// add an order of magnitude to be safe and round to 7x24h of blocks to make it
/// easier to reason about.
pub const CUT_THROUGH_HORIZON: u32 = WEEK_HEIGHT as u32;

/// Default number of blocks in the past to determine the height where we request
/// a txhashset (and full blocks from). Needs to be long enough to not overlap with
/// a long reorg.
/// Rational behind the value is the longest bitcoin fork was about 30 blocks, so 5h.
/// We add an order of magnitude to be safe and round to 2x24h of blocks to make it
/// easier to reason about.
pub const STATE_SYNC_THRESHOLD: u32 = 2 * DAY_HEIGHT as u32;

/// Weight of an input when counted against the max block weight capacity
pub const BLOCK_INPUT_WEIGHT: usize = 1;

/// Weight of an output when counted against the max block weight capacity
pub const BLOCK_OUTPUT_WEIGHT: usize = 21;

/// Weight of a kernel when counted against the max block weight capacity
pub const BLOCK_KERNEL_WEIGHT: usize = 3;

/// Total maximum block weight. At current sizes, this means a maximum
/// theoretical size of:
/// * `(674 + 33 + 1) * (40_000 / 21) = 1_348_571` for a block with only outputs
/// * `(1 + 8 + 8 + 33 + 64) * (40_000 / 3) = 1_520_000` for a block with only kernels
/// * `(1 + 33) * 40_000 = 1_360_000` for a block with only inputs
///
/// Regardless of the relative numbers of inputs/outputs/kernels in a block the maximum
/// block size is around 1.5MB
/// For a block full of "average" txs (2 inputs, 2 outputs, 1 kernel) we have -
/// `(1 * 2) + (21 * 2) + (3 * 1) = 47` (weight per tx)
/// `40_000 / 47 = 851` (txs per block)
///
pub const MAX_BLOCK_WEIGHT: usize = 40_000;

/// Fork every 6 months.
pub const HARD_FORK_INTERVAL: u64 = YEAR_HEIGHT / 2;

/// Floonet first hard fork height, set to happen around 2019-06-20
pub const FLOONET_FIRST_HARD_FORK: u64 = 185_040;

/// Check whether the block version is valid at a given height, implements
/// 6 months interval scheduled hard forks for the first 2 years.
pub fn valid_header_version(height: u64, version: HeaderVersion) -> bool {
	let chain_type = global::CHAIN_TYPE.read().clone();
	match chain_type {
		global::ChainTypes::Floonet => {
			if height < FLOONET_FIRST_HARD_FORK {
				version == HeaderVersion::default()
			// add branches one by one as we go from hard fork to hard fork
			// } else if height < FLOONET_SECOND_HARD_FORK {
			} else if height < 2 * HARD_FORK_INTERVAL {
				version == HeaderVersion::new(2)
			} else {
				false
			}
		}
		// everything else just like mainnet
		_ => {
			if height < HARD_FORK_INTERVAL {
				version == HeaderVersion::default()
			} else if height < 2 * HARD_FORK_INTERVAL {
				version == HeaderVersion::new(2)
			// uncomment branches one by one as we go from hard fork to hard fork
			/*} else if height < 3 * HARD_FORK_INTERVAL {
				version == HeaderVersion::new(3)
			} else if height < 4 * HARD_FORK_INTERVAL {
				version == HeaderVersion::new(4)
			} else {
				version > HeaderVersion::new(4) */
			} else {
				false
			}
		}
	}
}

/// Number of blocks used to calculate difficulty adjustments
pub const DIFFICULTY_ADJUST_WINDOW: u64 = HOUR_HEIGHT;

/// Average time span of the difficulty adjustment window
pub const BLOCK_TIME_WINDOW: u64 = DIFFICULTY_ADJUST_WINDOW * BLOCK_TIME_SEC;

/// Clamp factor to use for difficulty adjustment
/// Limit value to within this factor of goal
pub const CLAMP_FACTOR: u64 = 2;

/// Dampening factor to use for difficulty adjustment
pub const DIFFICULTY_DAMP_FACTOR: u64 = 3;

/// Dampening factor to use for AR scale calculation.
pub const AR_SCALE_DAMP_FACTOR: u64 = 13;

/// Compute weight of a graph as number of siphash bits defining the graph
/// Must be made dependent on height to phase out C31 in early 2020
/// Later phase outs are on hold for now
pub fn graph_weight(height: u64, edge_bits: u8) -> u64 {
	let mut xpr_edge_bits = edge_bits as u64;

	let bits_over_min = edge_bits.saturating_sub(global::min_edge_bits());
	let expiry_height = (1 << bits_over_min) * YEAR_HEIGHT;
	if edge_bits < 32 && height >= expiry_height {
		xpr_edge_bits = xpr_edge_bits.saturating_sub(1 + (height - expiry_height) / WEEK_HEIGHT);
	}

	(2 << (edge_bits - global::base_edge_bits()) as u64) * xpr_edge_bits
}

/// Minimum difficulty, enforced in diff retargetting
/// avoids getting stuck when trying to increase difficulty subject to dampening
pub const MIN_DIFFICULTY: u64 = DIFFICULTY_DAMP_FACTOR;

/// Minimum scaling factor for AR pow, enforced in diff retargetting
/// avoids getting stuck when trying to increase ar_scale subject to dampening
pub const MIN_AR_SCALE: u64 = AR_SCALE_DAMP_FACTOR;

/// unit difficulty, equal to graph_weight(SECOND_POW_EDGE_BITS)
pub const UNIT_DIFFICULTY: u64 =
	((2 as u64) << (SECOND_POW_EDGE_BITS - BASE_EDGE_BITS)) * (SECOND_POW_EDGE_BITS as u64);

/// The initial difficulty at launch. This should be over-estimated
/// and difficulty should come down at launch rather than up
/// Currently grossly over-estimated at 10% of current
/// ethereum GPUs (assuming 1GPU can solve a block at diff 1 in one block interval)
pub const INITIAL_DIFFICULTY: u64 = 1_000_000 * UNIT_DIFFICULTY;

/// Minimal header information required for the Difficulty calculation to
/// take place
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeaderInfo {
	/// Timestamp of the header, 1 when not used (returned info)
	pub timestamp: u64,
	/// Network difficulty or next difficulty to use
	pub difficulty: Difficulty,
	/// Network secondary PoW factor or factor to use
	pub secondary_scaling: u32,
	/// Whether the header is a secondary proof of work
	pub is_secondary: bool,
}

impl HeaderInfo {
	/// Default constructor
	pub fn new(
		timestamp: u64,
		difficulty: Difficulty,
		secondary_scaling: u32,
		is_secondary: bool,
	) -> HeaderInfo {
		HeaderInfo {
			timestamp,
			difficulty,
			secondary_scaling,
			is_secondary,
		}
	}

	/// Constructor from a timestamp and difficulty, setting a default secondary
	/// PoW factor
	pub fn from_ts_diff(timestamp: u64, difficulty: Difficulty) -> HeaderInfo {
		HeaderInfo {
			timestamp,
			difficulty,
			secondary_scaling: global::initial_graph_weight(),

			is_secondary: true,
		}
	}

	/// Constructor from a difficulty and secondary factor, setting a default
	/// timestamp
	pub fn from_diff_scaling(difficulty: Difficulty, secondary_scaling: u32) -> HeaderInfo {
		HeaderInfo {
			timestamp: 1,
			difficulty,
			secondary_scaling,
			is_secondary: true,
		}
	}
}

/// Move value linearly toward a goal
pub fn damp(actual: u64, goal: u64, damp_factor: u64) -> u64 {
	(actual + (damp_factor - 1) * goal) / damp_factor
}

/// limit value to be within some factor from a goal
pub fn clamp(actual: u64, goal: u64, clamp_factor: u64) -> u64 {
	max(goal / clamp_factor, min(actual, goal * clamp_factor))
}

/// Computes the proof-of-work difficulty that the next block should comply
/// with. Takes an iterator over past block headers information, from latest
/// (highest height) to oldest (lowest height).
///
/// The difficulty calculation is based on both Digishield and GravityWave
/// family of difficulty computation, coming to something very close to Zcash.
/// The reference difficulty is an average of the difficulty over a window of
/// DIFFICULTY_ADJUST_WINDOW blocks. The corresponding timespan is calculated
/// by using the difference between the median timestamps at the beginning
/// and the end of the window.
///
/// The secondary proof-of-work factor is calculated along the same lines, as
/// an adjustment on the deviation against the ideal value.
pub fn next_difficulty<T>(height: u64, cursor: T) -> HeaderInfo
where
	T: IntoIterator<Item = HeaderInfo>,
{
	// Create vector of difficulty data running from earliest
	// to latest, and pad with simulated pre-genesis data to allow earlier
	// adjustment if there isn't enough window data length will be
	// DIFFICULTY_ADJUST_WINDOW + 1 (for initial block time bound)
	let diff_data = global::difficulty_data_to_vector(cursor);

	// First, get the ratio of secondary PoW vs primary, skipping initial header
	let sec_pow_scaling = secondary_pow_scaling(height, &diff_data[1..]);

	// Get the timestamp delta across the window
	let ts_delta: u64 =
		diff_data[DIFFICULTY_ADJUST_WINDOW as usize].timestamp - diff_data[0].timestamp;

	// Get the difficulty sum of the last DIFFICULTY_ADJUST_WINDOW elements
	let diff_sum: u64 = diff_data
		.iter()
		.skip(1)
		.map(|dd| dd.difficulty.to_num())
		.sum();

	// adjust time delta toward goal subject to dampening and clamping
	let adj_ts = clamp(
		damp(ts_delta, BLOCK_TIME_WINDOW, DIFFICULTY_DAMP_FACTOR),
		BLOCK_TIME_WINDOW,
		CLAMP_FACTOR,
	);
	// minimum difficulty avoids getting stuck due to dampening
	let difficulty = max(MIN_DIFFICULTY, diff_sum * BLOCK_TIME_SEC / adj_ts);

	HeaderInfo::from_diff_scaling(Difficulty::from_num(difficulty), sec_pow_scaling)
}

/// Count, in units of 1/100 (a percent), the number of "secondary" (AR) blocks in the provided window of blocks.
pub fn ar_count(_height: u64, diff_data: &[HeaderInfo]) -> u64 {
	100 * diff_data.iter().filter(|n| n.is_secondary).count() as u64
}

/// Factor by which the secondary proof of work difficulty will be adjusted
pub fn secondary_pow_scaling(height: u64, diff_data: &[HeaderInfo]) -> u32 {
	// Get the scaling factor sum of the last DIFFICULTY_ADJUST_WINDOW elements
	let scale_sum: u64 = diff_data.iter().map(|dd| dd.secondary_scaling as u64).sum();

	// compute ideal 2nd_pow_fraction in pct and across window
	let target_pct = secondary_pow_ratio(height);
	let target_count = DIFFICULTY_ADJUST_WINDOW * target_pct;

	// Get the secondary count across the window, adjusting count toward goal
	// subject to dampening and clamping.
	let adj_count = clamp(
		damp(
			ar_count(height, diff_data),
			target_count,
			ar_scale_damp_factor(height),
		),
		target_count,
		CLAMP_FACTOR,
	);
	let scale = scale_sum * target_pct / max(1, adj_count);

	// minimum AR scale avoids getting stuck due to dampening
	max(MIN_AR_SCALE, scale) as u32
}

// MWC has block reward schedule similar to bitcoin
/// MWC Size of the block group
const MWC_BLOCKS_PER_GROUP: u64 = 2_100_000; // 4 years
const MWC_BLOCKS_PER_GROUP_FLOO: u64 = 2_100_000; // 4 years
/// MWC Block reward for the first group
pub const MWC_FIRST_GROUP_REWARD: u64 = 2_380_952_380;
const MWC_GROUPS_NUM: u64 = 32;
/// Calculate MWC block reward. The scedure is similar to bitcoints.
/// 1st 2.1 million blocks - 2.38095238 MWC
/// 2nd 2.1 million blocks - 1.19047619 MWC
/// 3rd 2.1 million blocks - 0.59523809 MWC
/// 4th 2.1 million blocks - 0.29761904 MWC
/// 5th 2.1 million blocks - 0.14880952 MWC
/// 6th 2.1 million blocks - 0.07440476 MWC
// ...
/// 32nd 2.1 million blocks - 0.000000001 MWC
//All blocks after that - 0 MWC (miner fees only)
pub fn calc_mwc_block_reward(height: u64) -> u64 {
	if height == 0 {
		// Genesis block
		return GENESIS_BLOCK_REWARD;
	}

	// Excluding the genesis block from any group
	let group_num = if global::is_floonet() {
		(height - 1) / MWC_BLOCKS_PER_GROUP_FLOO
	} else {
		(height - 1) / MWC_BLOCKS_PER_GROUP
	};

	if group_num >= MWC_GROUPS_NUM {
		0 // far far future, no rewards, sorry
	} else {
		let start_reward = MWC_FIRST_GROUP_REWARD;
		let group_div = 1 << group_num;
		start_reward / group_div
	}
}

/// MWC  calculate the total number of rewarded coins in all blocks including this one
pub fn calc_mwc_block_overage(height: u64, genesis_had_reward: bool) -> u64 {
	let blocks_per_group = if global::is_floonet() {
		MWC_BLOCKS_PER_GROUP_FLOO
	} else {
		MWC_BLOCKS_PER_GROUP
	};

	// including this one happens implicitly.
	// Because "this block is included", but 0 block (genesis) block is excluded, we will keep height as it is
	let mut block_count = height;
	let mut reward_per_block = MWC_FIRST_GROUP_REWARD;
	let mut overage: u64 = GENESIS_BLOCK_REWARD; // genesis block reward

	for _x in 0..MWC_GROUPS_NUM {
		overage += min(block_count, blocks_per_group) * reward_per_block;
		reward_per_block /= 2;

		if block_count < blocks_per_group {
			break;
		}

		block_count -= blocks_per_group;
	}

	if !genesis_had_reward {
		// Deducting the first block reward if it is 0. This case is used into the tests.
		overage -= GENESIS_BLOCK_REWARD;
	}

	overage
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn test_graph_weight() {
		// initial weights
		assert_eq!(graph_weight(1, 31), 256 * 31);
		assert_eq!(graph_weight(1, 32), 512 * 32);
		assert_eq!(graph_weight(1, 33), 1024 * 33);

		// one year in, 31 starts going down, the rest stays the same
		assert_eq!(graph_weight(YEAR_HEIGHT, 31), 256 * 30);
		assert_eq!(graph_weight(YEAR_HEIGHT, 32), 512 * 32);
		assert_eq!(graph_weight(YEAR_HEIGHT, 33), 1024 * 33);

		// 31 loses one factor per week
		assert_eq!(graph_weight(YEAR_HEIGHT + WEEK_HEIGHT, 31), 256 * 29);
		assert_eq!(graph_weight(YEAR_HEIGHT + 2 * WEEK_HEIGHT, 31), 256 * 28);
		assert_eq!(graph_weight(YEAR_HEIGHT + 32 * WEEK_HEIGHT, 31), 0);

		// 2 years in, 31 still at 0, 32 starts decreasing
		assert_eq!(graph_weight(2 * YEAR_HEIGHT, 31), 0);
		assert_eq!(graph_weight(2 * YEAR_HEIGHT, 32), 512 * 32);
		assert_eq!(graph_weight(2 * YEAR_HEIGHT, 33), 1024 * 33);

		// 32 phaseout on hold
		assert_eq!(graph_weight(2 * YEAR_HEIGHT + WEEK_HEIGHT, 32), 512 * 32);
		assert_eq!(graph_weight(2 * YEAR_HEIGHT + WEEK_HEIGHT, 31), 0);
		assert_eq!(
			graph_weight(2 * YEAR_HEIGHT + 30 * WEEK_HEIGHT, 32),
			512 * 32
		);
		assert_eq!(
			graph_weight(2 * YEAR_HEIGHT + 31 * WEEK_HEIGHT, 32),
			512 * 32
		);

		// 3 years in, nothing changes
		assert_eq!(graph_weight(3 * YEAR_HEIGHT, 31), 0);
		assert_eq!(graph_weight(3 * YEAR_HEIGHT, 32), 512 * 32);
		assert_eq!(graph_weight(3 * YEAR_HEIGHT, 33), 1024 * 33);

		// 4 years in, still on hold
		assert_eq!(graph_weight(4 * YEAR_HEIGHT, 31), 0);
		assert_eq!(graph_weight(4 * YEAR_HEIGHT, 32), 512 * 32);
		assert_eq!(graph_weight(4 * YEAR_HEIGHT, 33), 1024 * 33);
	}

	// MWC  testing calc_mwc_block_reward output for the scedule that documented at definition of calc_mwc_block_reward
	#[test]
	fn test_calc_mwc_block_reward() {
		// Code is crucial, so just checking all groups one by one manually.
		// We don't use the constants here because we can mess up with them as well.
		assert_eq!(
			calc_mwc_block_reward(0),
			10_000_000 * 1_000_000_000 + 41_800_000
		); // group 1, genesis 10M
		assert_eq!(calc_mwc_block_reward(1), 2_380_952_380); // group 1
		assert_eq!(calc_mwc_block_reward(2), 2_380_952_380); // group 1
		assert_eq!(calc_mwc_block_reward(2_100_000 - 1), 2_380_952_380); // group 1
        assert_eq!(calc_mwc_block_reward(2_100_000), 2_380_952_380); // group 1
		assert_eq!(
			calc_mwc_block_reward(MWC_BLOCKS_PER_GROUP - 1),
			MWC_FIRST_GROUP_REWARD
		); // group 1
		assert_eq!(
			calc_mwc_block_reward(MWC_BLOCKS_PER_GROUP),
            MWC_FIRST_GROUP_REWARD
        ); // group 1
		assert_eq!(calc_mwc_block_reward(2_100_000 + 1), 1_190_476_190); // group 2
		assert_eq!(
			calc_mwc_block_reward(MWC_BLOCKS_PER_GROUP + 1),
			MWC_FIRST_GROUP_REWARD / 2
		); // group 2
		assert_eq!(calc_mwc_block_reward(2_100_000 + 200), 1_190_476_190); // group 2
		assert_eq!(calc_mwc_block_reward(2_100_000 * 2 + 200), 595_238_095); // group 3
		assert_eq!(calc_mwc_block_reward(2_100_000 * 3 + 200), 297_619_047); // group 4
		assert_eq!(calc_mwc_block_reward(2_100_000 * 4 + 200), 148_809_523); // group 5
		assert_eq!(calc_mwc_block_reward(2_100_000 * 5 + 200), 74_404_761); // group 6
		assert_eq!(calc_mwc_block_reward(2_100_000 * 6 + 200), 37_202_380); // group 7
		assert_eq!(calc_mwc_block_reward(2_100_000 * 7 + 200), 18_601_190); // group 8
		assert_eq!(calc_mwc_block_reward(2_100_000 * 8 + 200), 9_300_595); // group 9
		assert_eq!(calc_mwc_block_reward(2_100_000 * 9 + 200), 4_650_297); // group 10
		assert_eq!(calc_mwc_block_reward(2_100_000 * 10 + 200), 2_325_148); // group 11
		assert_eq!(calc_mwc_block_reward(2_100_000 * 11 + 200), 1_162_574); // group 12
		assert_eq!(calc_mwc_block_reward(2_100_000 * 12 + 200), 581_287); // group 13
		assert_eq!(calc_mwc_block_reward(2_100_000 * 13 + 200), 290_643); // group 14
		assert_eq!(calc_mwc_block_reward(2_100_000 * 14 + 200), 145_321); // group 15
		assert_eq!(calc_mwc_block_reward(2_100_000 * 15 + 200), 72_660); // group 16
		assert_eq!(calc_mwc_block_reward(2_100_000 * 16 + 200), 36_330); // group 17
		assert_eq!(calc_mwc_block_reward(2_100_000 * 17 + 200), 18_165); // group 18
		assert_eq!(calc_mwc_block_reward(2_100_000 * 18 + 200), 9_082); // group 19
		assert_eq!(calc_mwc_block_reward(2_100_000 * 19 + 200), 4_541); // group 20
		assert_eq!(calc_mwc_block_reward(2_100_000 * 20 + 200), 2_270); // group 21
		assert_eq!(calc_mwc_block_reward(2_100_000 * 21 + 200), 1_135); // group 22
		assert_eq!(calc_mwc_block_reward(2_100_000 * 22 + 200), 567); // group 23
		assert_eq!(calc_mwc_block_reward(2_100_000 * 23 + 200), 283); // group 24
		assert_eq!(calc_mwc_block_reward(2_100_000 * 24 + 200), 141); // group 25
		assert_eq!(calc_mwc_block_reward(2_100_000 * 25 + 200), 70); // group 26
		assert_eq!(calc_mwc_block_reward(2_100_000 * 26 + 200), 35); // group 27
		assert_eq!(calc_mwc_block_reward(2_100_000 * 27 + 200), 17); // group 28
		assert_eq!(calc_mwc_block_reward(2_100_000 * 28 + 200), 8); // group 29
		assert_eq!(calc_mwc_block_reward(2_100_000 * 29 + 200), 4); // group 30
		assert_eq!(calc_mwc_block_reward(2_100_000 * 30 + 200), 2); // group 32
		assert_eq!(calc_mwc_block_reward(2_100_000 * 31 + 200), 1); // group 32
		assert_eq!(calc_mwc_block_reward(2_100_000 * 32 + 200), 0); // group 32+
		assert_eq!(calc_mwc_block_reward(2_100_000 * 320 + 200), 0); // group 32+
	}

	// MWC  testing calc_mwc_block_overage output for the schedule that documented at definition of calc_mwc_block_reward
	#[test]
	fn test_calc_mwc_block_overage() {
		let genesis_reward: u64 = GENESIS_BLOCK_REWARD;

		assert_eq!(calc_mwc_block_overage(0, true), genesis_reward); // Doesn't make sence to call for the genesis block
		assert_eq!(calc_mwc_block_overage(0, false), 0); // Doesn't make sence to call for the genesis block
		assert_eq!(
			calc_mwc_block_overage(1, true),
			genesis_reward + MWC_FIRST_GROUP_REWARD * 1
		);

		assert_eq!(
			calc_mwc_block_overage(30, true),
			genesis_reward + MWC_FIRST_GROUP_REWARD * 30
		);
		assert_eq!(
			calc_mwc_block_overage(30, false),
			MWC_FIRST_GROUP_REWARD * 30
		);
		// pre last block in the first group
		assert_eq!(
			calc_mwc_block_overage(MWC_BLOCKS_PER_GROUP - 1, true),
			genesis_reward + MWC_FIRST_GROUP_REWARD * (MWC_BLOCKS_PER_GROUP - 1)
		);
        // last block in the first group
		assert_eq!(
			calc_mwc_block_overage(MWC_BLOCKS_PER_GROUP, true),
			genesis_reward + MWC_FIRST_GROUP_REWARD * MWC_BLOCKS_PER_GROUP
		);
        // first block in the second group
        assert_eq!(
			calc_mwc_block_overage(MWC_BLOCKS_PER_GROUP + 1, true),
            genesis_reward
                + MWC_FIRST_GROUP_REWARD * MWC_BLOCKS_PER_GROUP
				+ MWC_FIRST_GROUP_REWARD / 2
		);

		assert_eq!(
			calc_mwc_block_overage(MWC_BLOCKS_PER_GROUP + 5000, true),
			genesis_reward
				+ MWC_FIRST_GROUP_REWARD * MWC_BLOCKS_PER_GROUP
				+ 5000 * MWC_FIRST_GROUP_REWARD / 2
		);

		// Calculating the total number of coins
		let total_blocks_reward = calc_mwc_block_overage(2_100_000_000 * 320, true);
		// Expected 20M in total. The coin base is exactly 20M
		assert_eq!(total_blocks_reward, 20_000_000 * GRIN_BASE);
	}
}
