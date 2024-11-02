// Copyright (C) 2024, Rhombus Tech. All rights reserved.
// See the file LICENSE for licensing terms.

pub mod types;
pub mod state;
pub mod core;
pub mod challenge;
pub mod external;
pub mod execution;

pub use types::*;
pub use state::*;

// Constants used throughout the contract
pub const MAX_GAS: wasmlanche::Gas = 10_000_000;
pub const ZERO: u64 = 0;
pub const TIMEOUT_INTERVAL: u64 = 15;
pub const CHALLENGE_RESPONSE_WINDOW: u64 = 100;
pub const MIN_WATCHDOGS: usize = 3;
