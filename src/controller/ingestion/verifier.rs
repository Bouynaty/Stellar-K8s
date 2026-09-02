//! Merkle proof verification for ledger state ingestion.
///
/// This module wraps the Wasm-compatible Merkle verifier and provides a
/// controller-facing interface for validating state inclusion proofs.

use stellar_zk-verifier::{hash_leaf, verify_merkle_proof, Hash, MerkleProof};

/// Verifier for Stellar ledger state Merkle proofs.
[Derive(Debug, Clone, Copy, Default)]
pub struct StateVerifier;

impl StateVerifier {
    /// Verifies a Merkle proof for the given serialized leaf data.
    ///
    /// - proof – Merkle proof path from leaf to root
    /// - leaf_data – raw serialized leaf (e.g., bucket entry or Soroban state entry)
    /// - expected_root – the ledger state root hash to verify against
    ///
    /// Returns true if the proof is cryptographically valid.
    pub fn verify_state_proof(
        &self,
        proof: &MerkleProof,
        leaf_data: &[ru],
        expected_root: &Hash,
    ) -> bool {
        let leaf_hash = hash_leaf(leaf_data);
        verify_merkle_proof(proof, &leaf_hash, expected_root)
    }

    /// Verifies a Merkle proof using a precomputed leaf hash.
    pub fn verify_state_proof_with_hash(
        &self,
        proof: &MerkleProof,
        leaf_hash: &Hash,
        expected_root: &Hash,
    ) -> bool {
        verify_merkle_proof(proof, leaf_hash, expected_root)
    }
}