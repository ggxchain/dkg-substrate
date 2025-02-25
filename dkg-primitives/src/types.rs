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
//
use codec::{Decode, Encode};
use curv::elliptic::curves::{Point, Scalar, Secp256k1};
use dkg_runtime_primitives::{crypto::AuthorityId, gossip_messages::*, SignerSetId};
use multi_party_ecdsa::protocols::multi_party_ecdsa::gg_2020::state_machine::keygen::LocalKey;
use sp_runtime::traits::{Block, Hash, Header};
use std::fmt;

pub use dkg_runtime_primitives::SessionId;

pub type FE = Scalar<Secp256k1>;
pub type GE = Point<Secp256k1>;

/// DKGMsgStatus Enum identifies if a message is for an active or queued round
#[derive(Debug, Copy, Clone, Decode, Encode)]
#[cfg_attr(feature = "scale-info", derive(scale_info::TypeInfo))]
pub enum DKGMsgStatus {
	/// Active round
	ACTIVE,
	/// Queued round,
	QUEUED,
}

pub type SSID = u16;

/// Gossip message struct for all DKG + Webb Protocol messages.
///
/// A message wrapper intended to be passed between the nodes
#[derive(Debug, Clone, Decode, Encode)]
#[cfg_attr(feature = "scale-info", derive(scale_info::TypeInfo))]
pub struct DKGMessage<AuthorityId> {
	/// Node authority id
	pub sender_id: AuthorityId,
	/// Authority id of the recipient.
	///
	/// If None, the message is broadcasted to all nodes.
	pub recipient_id: Option<AuthorityId>,
	/// DKG message contents
	pub payload: NetworkMsgPayload,
	/// Identifier for the message
	pub session_id: SessionId,
	/// The round ID
	pub associated_block_id: u64,
	/// The signing set ID
	pub ssid: SSID,
}

#[derive(Debug, Clone, Decode, Encode)]
#[cfg_attr(feature = "scale-info", derive(scale_info::TypeInfo))]
pub struct SignedDKGMessage<AuthorityId> {
	/// DKG messages
	pub msg: DKGMessage<AuthorityId>,
	/// ECDSA signature of sha3(concatenated message contents)
	pub signature: Option<Vec<u8>>,
}

impl<AuthorityId> SignedDKGMessage<AuthorityId> {
	pub fn message_hash<B: Block>(&self) -> B::Hash
	where
		DKGMessage<AuthorityId>: Encode,
	{
		// in case of Keygen or Offline, we only need to hash the inner raw message bytes that
		// are going to be sent to the state machine.
		let bytes_to_hash = match self.msg.payload {
			NetworkMsgPayload::Keygen(ref m) => m.keygen_msg.clone(),
			NetworkMsgPayload::Offline(ref m) => m.offline_msg.clone(),
			NetworkMsgPayload::Vote(ref m) => m.encode(),
			NetworkMsgPayload::PublicKeyBroadcast(ref m) => m.encode(),
			NetworkMsgPayload::MisbehaviourBroadcast(ref m) => m.encode(),
		};
		<<B::Header as Header>::Hashing as Hash>::hash_of(&bytes_to_hash)
	}
}

impl<ID> fmt::Display for DKGMessage<ID> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let label = match self.payload {
			NetworkMsgPayload::Keygen(_) => "Keygen",
			NetworkMsgPayload::Offline(_) => "Offline",
			NetworkMsgPayload::Vote(_) => "Vote",
			NetworkMsgPayload::PublicKeyBroadcast(_) => "PublicKeyBroadcast",
			NetworkMsgPayload::MisbehaviourBroadcast(_) => "MisbehaviourBroadcast",
		};
		write!(f, "DKGMessage of type {label}")
	}
}

#[derive(Debug, Clone, Decode, Encode)]
#[cfg_attr(feature = "scale-info", derive(scale_info::TypeInfo))]
pub enum NetworkMsgPayload {
	Keygen(DKGKeygenMessage),
	Offline(DKGOfflineMessage),
	Vote(DKGVoteMessage),
	PublicKeyBroadcast(PublicKeyMessage),
	MisbehaviourBroadcast(MisbehaviourMessage),
}

impl NetworkMsgPayload {
	pub fn payload(&self) -> &Vec<u8> {
		match self {
			NetworkMsgPayload::Offline(msg) => &msg.offline_msg,
			NetworkMsgPayload::Vote(msg) => &msg.partial_signature,
			NetworkMsgPayload::Keygen(msg) => &msg.keygen_msg,
			NetworkMsgPayload::PublicKeyBroadcast(msg) => &msg.pub_key,
			NetworkMsgPayload::MisbehaviourBroadcast(msg) => &msg.signature,
		}
	}
	pub fn unsigned_proposal_hash(&self) -> Option<&[u8; 32]> {
		match self {
			NetworkMsgPayload::Offline(msg) => Some(&msg.unsigned_proposal_hash),
			NetworkMsgPayload::Vote(msg) => Some(&msg.unsigned_proposal_hash),
			_ => None,
		}
	}

	pub fn keygen_protocol_hash(&self) -> Option<&[u8; 32]> {
		if let NetworkMsgPayload::Keygen(msg) = self {
			Some(&msg.keygen_protocol_hash)
		} else {
			None
		}
	}
	/// NOTE: this is hacky
	/// TODO: Change enums for keygen, offline, vote
	pub fn async_proto_only_get_sender_id(&self) -> Option<u16> {
		match self {
			NetworkMsgPayload::Keygen(kg) => Some(kg.sender_id),
			NetworkMsgPayload::Offline(offline) => Some(offline.signer_set_id as u16),
			NetworkMsgPayload::Vote(vote) => Some(vote.party_ind),
			_ => None,
		}
	}

	pub fn get_type(&self) -> &'static str {
		match self {
			NetworkMsgPayload::Keygen(_) => "keygen",
			NetworkMsgPayload::Offline(_) => "offline",
			NetworkMsgPayload::Vote(_) => "vote",
			NetworkMsgPayload::PublicKeyBroadcast(_) => "pub_key_broadcast",
			NetworkMsgPayload::MisbehaviourBroadcast(_) => "misbehaviour",
		}
	}
}

pub trait DKGRoundsSM<Payload, Output, Clock> {
	fn proceed(&mut self, _at: Clock) -> Result<bool, DKGError> {
		Ok(false)
	}

	fn get_outgoing(&mut self) -> Vec<Payload> {
		vec![]
	}

	fn handle_incoming(&mut self, _data: Payload, _at: Clock) -> Result<(), DKGError> {
		Ok(())
	}

	fn is_finished(&self) -> bool {
		false
	}

	fn try_finish(self) -> Result<Output, DKGError>;
}

pub struct KeygenParams {
	pub session_id: SessionId,
	pub party_index: u16,
	pub threshold: u16,
	pub parties: u16,
}

pub struct SignParams {
	pub session_id: SessionId,
	pub party_index: u16,
	pub threshold: u16,
	pub parties: u16,
	pub signers: Vec<u16>,
	pub signer_set_id: SignerSetId,
}

#[derive(Debug, Clone)]
pub enum DKGResult {
	Empty,
	KeygenFinished { session_id: SessionId, local_key: Box<LocalKey<Secp256k1>> },
}

#[derive(Debug, Clone)]
pub enum DKGError {
	KeygenMisbehaviour { reason: String, bad_actors: Vec<AuthorityId>, session_id: SessionId },
	KeygenTimeout { session_id: SessionId, bad_actors: Vec<AuthorityId> },
	StartKeygen { reason: String },
	StartOffline { reason: String },
	CreateOfflineStage { reason: String },
	Vote { reason: String },
	CriticalError { reason: String },
	GenericError { reason: String }, // TODO: handle other
	NoAuthorityAccounts,
	NoHeader,
	SignMisbehaviour { reason: String, bad_actors: Vec<AuthorityId>, session_id: SessionId },
	InvalidPeerId,
	InvalidSignature,
	InvalidKeygenPartyId,
	InvalidSigningSet,
	InputOutOfBounds,
	CannotSign,
}

impl fmt::Display for DKGError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		use DKGError::*;
		let label = match self {
			KeygenMisbehaviour { bad_actors, reason, .. } =>
				format!("Keygen misbehaviour: reason: {reason} bad actors: {bad_actors:?}"),
			KeygenTimeout { bad_actors, session_id } =>
				format!("Keygen timeout @ Session({session_id}): bad actors: {bad_actors:?}"),
			Vote { reason } => format!("Vote: {reason}"),
			StartKeygen { reason } => format!("Start keygen: {reason}"),
			CreateOfflineStage { reason } => format!("Create offline stage: {reason}"),
			CriticalError { reason } => format!("Critical error: {reason}"),
			GenericError { reason } => format!("Generic error: {reason}"),
			StartOffline { reason } => format!("Unable to start Offline Signing: {reason}"),
			NoAuthorityAccounts => "No Authority accounts found!".to_string(),
			NoHeader => "No Header!".to_string(),
			SignMisbehaviour { reason, bad_actors, .. } =>
				format!("SignMisbehaviour: reason: {reason},  bad actors: {bad_actors:?}"),
			InvalidPeerId => "Invalid PeerId!".to_string(),
			InvalidSignature => "Invalid Signature!".to_string(),
			InvalidKeygenPartyId => "Invalid Keygen Party Id!".to_string(),
			InvalidSigningSet => "Invalid Signing Set!".to_string(),
			InputOutOfBounds => "Input value out of bounds set by runtime".to_string(),
			CannotSign => "Could not sign public key".to_string(),
		};
		write!(f, "DKGError of type {label}")
	}
}
