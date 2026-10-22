//! The membership contract suite (5 cases) on `mem`: flooding on top of `MemNet`.

use super::test_support::mem_sync_pair;
use crate::transport::net::contract::membership::membership_contract;

membership_contract!(mem_sync_pair);
