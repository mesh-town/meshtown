//! Replica contracts (21 cases) and membership (5 cases) on `mem`: its own replica on top of flooding.

use super::test_support::mesh_mem_pair;
use crate::net::contract::membership::membership_contract;
use crate::net::contract::replica::replica_contract;

replica_contract!(mesh_mem_pair);
membership_contract!(mesh_mem_pair);
