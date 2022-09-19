// This file is part of Webb.

// Copyright (C) 2021 Webb Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Bridge Registry Module
//!
//! A module for maintaining bridge metadata or views over connected
//! sets of anchors.
//!
//! ## Overview
//!
//! The Bridge Registry module provides functionality maintaing and storing
//! metadata about existing bridges.
//!
//! The supported dispatchable functions are documented in the [`Call`] enum.
//!
//! ### Terminology
//!
//! ### Goals
//!
//! ## Interface
//!
//! ## Related Modules
//!
//! * [`System`](../frame_system/index.html)
//! * [`Support`](../frame_support/index.html)

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

mod benchmarking;
mod types;

mod weights;
use weights::WeightInfo;

use types::*;

use sp_std::{convert::TryInto, prelude::*};

use frame_support::pallet_prelude::{ensure, DispatchError};
// pub use pallet::*;
use dkg_runtime_primitives::{BridgeRegistryTrait, ProposalHandlerTrait};
use pallet_eth2_light_client::Eth2Prover;
use sp_runtime::traits::{AtLeast32Bit, One, Zero};
use webb_proposals::{
	evm::AnchorUpdateProposal, FunctionSignature, OnSignedProposal, Proposal, ProposalHeader,
	ProposalKind, ResourceId, TargetSystem, TypedChainId,
};

use eth_types::{LogEntry, Receipt};

use rlp::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{dispatch::DispatchResultWithPostInfo, pallet_prelude::*};
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	/// The module configuration trait.
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Weight information for the extrinsics
		type WeightInfo: WeightInfo;

		type Eth2Prover: Eth2Prover;

		type BridgeRegistry: BridgeRegistryTrait;

		type ProposalHandler: ProposalHandlerTrait;
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub phantom: (PhantomData<T>),
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { phantom: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {}
	}

	#[pallet::event]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(100)]
		pub fn submit_anchor_update_proposal(
			origin: OriginFor<T>,
			typed_chain_id: TypedChainId,
			proof_data: ProofData,
		) -> DispatchResultWithPostInfo {
			match typed_chain_id {
				TypedChainId::Evm(_) => {
					let evm_proof_data = proof_data.to_evm_proof().unwrap();
					ensure!(
						Eth2Prover::verify_log_entry(
							evm_proof_data.log_index,
							evm_proof_data.log_entry_data,
							evm_proof_data.receipt_index,
							evm_proof_data.receipt_data,
							evm_proof_data.header_data,
							evm_proof_data.proof,
						),
						Error::<T>::InvalidEvmLogEntryProof
					);
					// Submit the AnchorUpdateProposal
					Self::submit_anchor_update_proposals_evm(
						typed_chain_id,
						evm_proof_data.log_entry_data,
					);
				},
				_ => Err(Error::<T>::InvalidTypedChainId),
			}
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn submit_anchor_update_proposals_evm(
			typed_chain_id: TypedChainId,
			log_entry_data: Vec<u8>,
		) {
			let (contract_address, merkle_root, nonce) = Self::parse_evm_log(log_entry_data)?;
			let src_r_id =
				ResourceId::new(TargetSystem::ContractAddress(contract_address), typed_chain_id);
			let bridge =
				T::BridgeRegistry::bridges(T::BridgeRegistry::resource_to_bridge_index(src_r_id));
			for r in bridge.resource_ids {
				if r == src_r_id {
					continue
				}
				let function_sig = FunctionSignature::from([0u8; 4]);
				let proposal_header = ProposalHeader::new(r, function_sig, nonce);
				let proposal = AnchorUpdateProposal::new(proposal_header, merkle_root, src_r_id);
				T::ProposalHandler::submit_unsigned_proposal(typed_chain_id, proposal)?;
			}
		}

		pub fn parse_evm_log(log_entry_data: Vec<u8>) {
			let parsed_log: LogEntry = rlp::decode(log_entry_data.as_slice()).unwrap();

			let address = parsed_log.address.to_fixed_bytes();
			let topics = parsed_log.topics;
			let merkle_root = topics[1].to_fixed_bytes();
			let nonce = u32::from_be_bytes(topics[2].as_bytes());
			(address, merkle_root, nonce)
		}
	}
}
