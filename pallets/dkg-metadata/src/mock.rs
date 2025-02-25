// Copyright 2022 Webb Technologies Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// construct_runtime requires this
#![allow(clippy::from_over_into, clippy::unwrap_used)]
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	construct_runtime, parameter_types, sp_io::TestExternalities, BasicExternalities,
};
use frame_system::EnsureRoot;
use sp_core::{
	sr25519::{self, Signature},
	H256,
};
use sp_keystore::{testing::MemoryKeystore, KeystoreExt, KeystorePtr};
use sp_runtime::{
	app_crypto::ecdsa::Public,
	impl_opaque_keys,
	testing::TestXt,
	traits::{
		BlakeTwo256, Convert, ConvertInto, Extrinsic as ExtrinsicT, IdentifyAccount,
		IdentityLookup, OpaqueKeys, Verify,
	},
	BuildStorage, Percent, Permill,
};
use std::{sync::Arc, vec};

use crate as pallet_dkg_metadata;
pub use dkg_runtime_primitives::{
	crypto::AuthorityId as DKGId, ConsensusLog, MaxAuthorities, MaxKeyLength, MaxProposalLength,
	MaxReporters, MaxSignatureLength, DKG_ENGINE_ID,
};

impl_opaque_keys! {
	pub struct MockSessionKeys {
		pub dummy: pallet_dkg_metadata::Pallet<Test>,
	}
}

construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Timestamp: pallet_timestamp,
		DKGMetadata: pallet_dkg_metadata,
		Session: pallet_session,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type Nonce = u64;
	type Block = frame_system::mocking::MockBlock<Test>;
	type RuntimeCall = RuntimeCall;
	type Hashing = BlakeTwo256;
	type Hash = H256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

type Extrinsic = TestXt<RuntimeCall, ()>;

impl frame_system::offchain::SigningTypes for Test {
	type Public = <Signature as Verify>::Signer;
	type Signature = Signature;
}

impl<LocalCall> frame_system::offchain::SendTransactionTypes<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	type OverarchingCall = RuntimeCall;
	type Extrinsic = Extrinsic;
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
		call: RuntimeCall,
		_public: <Signature as Verify>::Signer,
		_account: AccountId,
		nonce: u64,
	) -> Option<(RuntimeCall, <Extrinsic as ExtrinsicT>::SignaturePayload)> {
		Some((call, (nonce, ())))
	}
}

pub const MILLISECS_PER_BLOCK: u64 = 10000;
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Test {
	type MinimumPeriod = MinimumPeriod;
	type Moment = u64;
	type OnTimestampSet = ();
	type WeightInfo = ();
}

parameter_types! {
	#[derive(Default, Clone, Encode, Decode, Debug, Eq, PartialEq, scale_info::TypeInfo, Ord, PartialOrd, MaxEncodedLen)]
	pub const VoteLength: u32 = 64;
}

impl pallet_dkg_metadata::Config for Test {
	type DKGId = DKGId;
	type DKGAuthorityToMerkleLeaf = DKGEcdsaToEthereumAddress;
	type RuntimeEvent = RuntimeEvent;
	type OnAuthoritySetChangeHandler = ();
	type OnDKGPublicKeyChangeHandler = ();
	type OffChainAuthId = dkg_runtime_primitives::offchain::crypto::OffchainAuthId;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type ForceOrigin = EnsureRoot<Self::AccountId>;
	type KeygenJailSentence = Period;
	type SigningJailSentence = Period;
	type DecayPercentage = DecayPercentage;
	type Reputation = u128;
	type UnsignedInterval = frame_support::traits::ConstU64<0>;
	type UnsignedPriority = frame_support::traits::ConstU64<1000>;
	type AuthorityIdOf = pallet_dkg_metadata::AuthorityIdOf<Self>;
	type ProposalHandler = ();
	type SessionPeriod = Period;
	type MaxKeyLength = MaxKeyLength;
	type MaxSignatureLength = MaxSignatureLength;
	type MaxReporters = MaxReporters;
	type MaxAuthorities = MaxAuthorities;
	type VoteLength = VoteLength;
	type MaxProposalLength = MaxProposalLength;
	type WeightInfo = ();
}

parameter_types! {
	pub const DecayPercentage: Percent = Percent::from_percent(50);
	pub const Period: u64 = 1;
	pub const Offset: u64 = 0;
	pub const RefreshDelay: Permill = Permill::from_percent(90);
	pub const TimeToRestart: u64 = 3;
}

impl pallet_session::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = AccountId;
	type ValidatorIdOf = ConvertInto;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type SessionManager = MockSessionManager;
	type SessionHandler = <MockSessionKeys as OpaqueKeys>::KeyTypeIdProviders;
	type Keys = MockSessionKeys;
	type WeightInfo = ();
}

pub struct MockSessionManager;

impl pallet_session::SessionManager<AccountId> for MockSessionManager {
	fn end_session(_: sp_staking::SessionIndex) {}
	fn start_session(_: sp_staking::SessionIndex) {}
	fn new_session(idx: sp_staking::SessionIndex) -> Option<Vec<AccountId>> {
		if idx == 0 || idx == 1 {
			Some(vec![mock_pub_key(1), mock_pub_key(2)])
		} else if idx == 2 {
			Some(vec![mock_pub_key(3), mock_pub_key(4)])
		} else {
			None
		}
	}
}

// Note, that we can't use `UintAuthorityId` here. Reason is that the implementation
// of `to_public_key()` assumes, that a public key is 32 bytes long. This is true for
// ed25519 and sr25519 but *not* for ecdsa. An ecdsa public key is 33 bytes.
pub fn mock_dkg_id(id: u8) -> DKGId {
	DKGId::from(Public::from_raw([id; 33]))
}

pub fn mock_pub_key(id: u8) -> AccountId {
	sr25519::Public::from_raw([id; 32])
}

pub fn mock_authorities(vec: Vec<u8>) -> Vec<(AccountId, DKGId)> {
	vec.into_iter().map(|id| (mock_pub_key(id), mock_dkg_id(id))).collect()
}

pub fn new_test_ext(ids: Vec<u8>) -> TestExternalities {
	new_test_ext_raw_authorities(mock_authorities(ids))
}

pub fn new_test_ext_raw_authorities(authorities: Vec<(AccountId, DKGId)>) -> TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	let session_keys: Vec<_> = authorities
		.iter()
		.enumerate()
		.map(|(_, id)| (id.0, id.0, MockSessionKeys { dummy: id.1.clone() }))
		.collect();

	BasicExternalities::execute_with_storage(&mut t, || {
		for (ref id, ..) in &session_keys {
			frame_system::Pallet::<Test>::inc_providers(id);
		}
	});

	pallet_session::GenesisConfig::<Test> { keys: session_keys }
		.assimilate_storage(&mut t)
		.unwrap();

	pallet_dkg_metadata::GenesisConfig::<Test> {
		authorities: authorities.clone().into_iter().map(|(_, id)| id).collect(),
		signature_threshold: 1,
		keygen_threshold: 3,
		authority_ids: authorities.into_iter().map(|(acc, _id)| acc).collect(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	// set to block 1 to test events
	ext.execute_with(|| System::set_block_number(1));
	ext.register_extension(KeystoreExt(Arc::new(MemoryKeystore::new()) as KeystorePtr));
	ext
}

/// Convert DKG secp256k1 public keys into Ethereum addresses
pub struct DKGEcdsaToEthereumAddress;
impl Convert<dkg_runtime_primitives::crypto::AuthorityId, Vec<u8>> for DKGEcdsaToEthereumAddress {
	fn convert(a: dkg_runtime_primitives::crypto::AuthorityId) -> Vec<u8> {
		use k256::{ecdsa::VerifyingKey, elliptic_curve::sec1::ToEncodedPoint};
		let _x = VerifyingKey::from_sec1_bytes(sp_core::crypto::ByteArray::as_slice(&a));
		VerifyingKey::from_sec1_bytes(sp_core::crypto::ByteArray::as_slice(&a))
			.map(|pub_key| {
				// uncompress the key
				let uncompressed = pub_key.to_encoded_point(false);
				// convert to ETH address
				sp_io::hashing::keccak_256(&uncompressed.as_bytes()[1..])[12..].to_vec()
			})
			.map_err(|_| {
				log::error!(target: "runtime::dkg_proposals", "Invalid DKG PublicKey format!");
			})
			.unwrap_or_default()
	}
}
