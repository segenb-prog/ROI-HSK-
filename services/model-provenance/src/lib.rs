//! AI Model Provenance - Training Data Lineage Tracking
//! 
//! Tracks which data (and which consents) were used to train AI models,
//! enabling verification that models were/weren't trained on specific data.

use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use std::collections::{HashMap, HashSet};

/// Model checkpoint with provenance information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelCheckpoint {
    /// Unique checkpoint identifier
    pub checkpoint_id: String,
    /// Model version
    pub version: String,
    /// Training iteration/epoch
    pub iteration: u64,
    /// Hash of model weights
    pub model_hash: [u8; 32],
    /// Merkle root of training data used
    pub training_data_root: [u8; 32],
    /// Merkle root of consent proofs
    pub consent_root: [u8; 32],
    /// Timestamp
    pub timestamp: u64,
    /// Parent checkpoint (for lineage)
    pub parent_checkpoint: Option<String>,
    /// Hyperparameters hash
    pub hyperparameters_hash: [u8; 32],
}

/// Training batch record
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrainingBatch {
    /// Batch identifier
    pub batch_id: String,
    /// Data samples in this batch
    pub data_samples: Vec<DataSample>,
    /// Consent IDs for this batch
    pub consent_ids: Vec<String>,
    /// Merkle root of batch
    pub batch_root: [u8; 32],
    /// Gradient update hash
    pub gradient_hash: [u8; 32],
}

/// Individual data sample
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataSample {
    /// Sample identifier (hashed for privacy)
    pub sample_hash: [u8; 32],
    /// Data category
    pub category: String,
    /// Associated consent IDs
    pub consent_ids: Vec<String>,
    /// Timestamp when added
    pub added_at: u64,
}

/// Proof that data was used in training
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrainingInclusionProof {
    /// Checkpoint ID
    pub checkpoint_id: String,
    /// Data sample hash
    pub sample_hash: [u8; 32],
    /// Merkle proof path
    pub merkle_proof: Vec<[u8; 32]>,
    /// Index in tree
    pub leaf_index: usize,
    /// Consent verification
    pub consent_verification: ConsentVerification,
}

/// Proof that data was NOT used in training
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrainingExclusionProof {
    /// Checkpoint ID
    pub checkpoint_id: String,
    /// Data sample hash
    pub sample_hash: [u8; 32],
    /// Sorted list of all sample hashes in checkpoint
    pub neighbor_hashes: Vec<[u8; 32]>,
    /// Proof of absence (show where sample would be if present)
    pub absence_proof: AbsenceProof,
}

/// Consent verification for training
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsentVerification {
    /// Consent ID
    pub consent_id: String,
    /// Consent signature
    pub signature: Vec<u8>,
    /// Verification timestamp
    pub verified_at: u64,
    /// Consent scope
    pub scope: String,
}

/// Proof of absence in sorted Merkle tree
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AbsenceProof {
    /// Index where sample would be inserted
    pub insertion_index: usize,
    /// Hash at insertion index (if exists)
    pub left_neighbor: Option<[u8; 32]>,
    /// Hash after insertion index (if exists)
    pub right_neighbor: Option<[u8; 32]>,
    /// Merkle proof for neighbors
    pub neighbor_proofs: Vec<Vec<[u8; 32]>>,
}

/// Model provenance tracker
pub struct ModelProvenance {
    /// All checkpoints
    checkpoints: HashMap<String, ModelCheckpoint>,
    /// Training data Merkle trees per checkpoint
    data_trees: HashMap<String, MerkleTree>,
    /// Consent trees per checkpoint
    consent_trees: HashMap<String, MerkleTree>,
    /// Sample to checkpoint mapping
    sample_index: HashMap<[u8; 32], HashSet<String>>,
}

/// Merkle tree for batch verification
#[derive(Clone, Debug)]
pub struct MerkleTree {
    pub leaves: Vec<[u8; 32]>,
    pub root: [u8; 32],
    pub layers: Vec<Vec<[u8; 32]>>,
}

impl MerkleTree {
    /// Build Merkle tree from leaves
    pub fn build(leaves: Vec<[u8; 32]>) -> Self {
        if leaves.is_empty() {
            return MerkleTree {
                leaves: vec![],
                root: [0u8; 32],
                layers: vec![],
            };
        }
        
        let mut layers: Vec<Vec<[u8; 32]>> = vec![leaves.clone()];
        let mut current_layer = leaves.clone();
        
        while current_layer.len() > 1 {
            let mut next_layer = Vec::new();
            
            for chunk in current_layer.chunks(2) {
                let left = chunk[0];
                let right = if chunk.len() > 1 { chunk[1] } else { chunk[0] };
                
                let parent = hash_pair(&left, &right);
                next_layer.push(parent);
            }
            
            layers.push(next_layer.clone());
            current_layer = next_layer;
        }
        
        let root = current_layer[0];
        
        MerkleTree {
            leaves,
            root,
            layers,
        }
    }
    
    /// Generate inclusion proof
    pub fn prove_inclusion(&self, index: usize) -> Option<Vec<[u8; 32]>> {
        if index >= self.leaves.len() {
            return None;
        }
        
        let mut proof = Vec::new();
        let mut current_index = index;
        
        for layer in &self.layers[..self.layers.len() - 1] {
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };
            
            if sibling_index < layer.len() {
                proof.push(layer[sibling_index]);
            }
            
            current_index /= 2;
        }
        
        Some(proof)
    }
    
    /// Verify inclusion proof
    pub fn verify_proof(&self, leaf: &[u8; 32], index: usize, proof: &[[u8; 32]]) -> bool {
        let mut current = *leaf;
        let mut current_index = index;
        
        for sibling in proof {
            current = if current_index % 2 == 0 {
                hash_pair(&current, sibling)
            } else {
                hash_pair(sibling, &current)
            };
            current_index /= 2;
        }
        
        current == self.root
    }
    
    /// Generate exclusion proof
    pub fn prove_exclusion(&self, sample_hash: &[u8; 32]) -> Option<AbsenceProof> {
        // Check if already present
        if self.leaves.contains(sample_hash) {
            return None;
        }
        
        // Find insertion point
        let mut insertion_index = 0;
        for (i, leaf) in self.leaves.iter().enumerate() {
            if leaf > sample_hash {
                insertion_index = i;
                break;
            }
            insertion_index = i + 1;
        }
        
        let left_neighbor = if insertion_index > 0 {
            Some(self.leaves[insertion_index - 1])
        } else {
            None
        };
        
        let right_neighbor = if insertion_index < self.leaves.len() {
            Some(self.leaves[insertion_index])
        } else {
            None
        };
        
        // Generate proofs for neighbors
        let mut neighbor_proofs = Vec::new();
        if let Some(_) = left_neighbor {
            neighbor_proofs.push(self.prove_inclusion(insertion_index.saturating_sub(1)).unwrap_or_default());
        }
        if right_neighbor.is_some() && insertion_index < self.leaves.len() {
            neighbor_proofs.push(self.prove_inclusion(insertion_index).unwrap_or_default());
        }
        
        Some(AbsenceProof {
            insertion_index,
            left_neighbor,
            right_neighbor,
            neighbor_proofs,
        })
    }
}

/// Hash two values together
fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

impl ModelProvenance {
    /// Create new provenance tracker
    pub fn new() -> Self {
        ModelProvenance {
            checkpoints: HashMap::new(),
            data_trees: HashMap::new(),
            consent_trees: HashMap::new(),
            sample_index: HashMap::new(),
        }
    }
    
    /// Record a new training checkpoint
    pub fn record_checkpoint(
        &mut self,
        checkpoint_id: String,
        version: String,
        iteration: u64,
        batches: &[TrainingBatch],
        parent: Option<String>,
    ) -> ModelCheckpoint {
        // Build data Merkle tree
        let mut all_samples: Vec<DataSample> = Vec::new();
        let mut all_consents: HashSet<String> = HashSet::new();
        
        for batch in batches {
            all_samples.extend(batch.data_samples.clone());
            all_consents.extend(batch.consent_ids.clone());
        }
        
        // Sort samples for deterministic tree
        all_samples.sort_by(|a, b| a.sample_hash.cmp(&b.sample_hash));
        
        let data_leaves: Vec<[u8; 32]> = all_samples.iter()
            .map(|s| s.sample_hash)
            .collect();
        
        let data_tree = MerkleTree::build(data_leaves);
        
        // Build consent tree
        let mut consent_list: Vec<String> = all_consents.into_iter().collect();
        consent_list.sort();
        
        let consent_leaves: Vec<[u8; 32]> = consent_list.iter()
            .map(|c| {
                let mut hasher = Sha256::new();
                hasher.update(c.as_bytes());
                hasher.finalize().into()
            })
            .collect();
        
        let consent_tree = MerkleTree::build(consent_leaves);
        
        // Compute model hash (simplified)
        let model_hash = compute_model_hash(checkpoint_id.clone(), iteration);
        
        // Create checkpoint
        let checkpoint = ModelCheckpoint {
            checkpoint_id: checkpoint_id.clone(),
            version,
            iteration,
            model_hash,
            training_data_root: data_tree.root,
            consent_root: consent_tree.root,
            timestamp: current_timestamp(),
            parent_checkpoint: parent,
            hyperparameters_hash: [0u8; 32], // Would be actual hash
        };
        
        // Update index
        for sample in &all_samples {
            self.sample_index
                .entry(sample.sample_hash)
                .or_insert_with(HashSet::new)
                .insert(checkpoint_id.clone());
        }
        
        // Store
        self.checkpoints.insert(checkpoint_id.clone(), checkpoint.clone());
        self.data_trees.insert(checkpoint_id.clone(), data_tree);
        self.consent_trees.insert(checkpoint_id, consent_tree);
        
        checkpoint
    }
    
    /// Prove that data was used in training
    pub fn prove_inclusion(
        &self,
        checkpoint_id: &str,
        sample_hash: &[u8; 32],
    ) -> Option<TrainingInclusionProof> {
        let checkpoint = self.checkpoints.get(checkpoint_id)?;
        let data_tree = self.data_trees.get(checkpoint_id)?;
        
        // Find sample index
        let index = data_tree.leaves.iter().position(|h| h == sample_hash)?;
        
        // Generate proof
        let merkle_proof = data_tree.prove_inclusion(index)?;
        
        Some(TrainingInclusionProof {
            checkpoint_id: checkpoint_id.to_string(),
            sample_hash: *sample_hash,
            merkle_proof,
            leaf_index: index,
            consent_verification: ConsentVerification {
                consent_id: format!("consent:{}", hex::encode(sample_hash)),
                signature: vec![],
                verified_at: checkpoint.timestamp,
                scope: "model_training".to_string(),
            },
        })
    }
    
    /// Prove that data was NOT used in training
    pub fn prove_exclusion(
        &self,
        checkpoint_id: &str,
        sample_hash: &[u8; 32],
    ) -> Option<TrainingExclusionProof> {
        let checkpoint = self.checkpoints.get(checkpoint_id)?;
        let data_tree = self.data_trees.get(checkpoint_id)?;
        
        // Check if present
        if data_tree.leaves.contains(sample_hash) {
            return None; // Cannot prove exclusion if present
        }
        
        // Generate exclusion proof
        let absence_proof = data_tree.prove_exclusion(sample_hash)?;
        
        // Get neighbor hashes
        let mut neighbor_hashes = Vec::new();
        if let Some(left) = absence_proof.left_neighbor {
            neighbor_hashes.push(left);
        }
        if let Some(right) = absence_proof.right_neighbor {
            neighbor_hashes.push(right);
        }
        
        Some(TrainingExclusionProof {
            checkpoint_id: checkpoint_id.to_string(),
            sample_hash: *sample_hash,
            neighbor_hashes,
            absence_proof,
        })
    }
    
    /// Verify inclusion proof
    pub fn verify_inclusion(&self, proof: &TrainingInclusionProof) -> bool {
        let Some(checkpoint) = self.checkpoints.get(&proof.checkpoint_id) else {
            return false;
        };
        
        let Some(data_tree) = self.data_trees.get(&proof.checkpoint_id) else {
            return false;
        };
        
        // Verify Merkle proof
        if !data_tree.verify_proof(&proof.sample_hash, proof.leaf_index, &proof.merkle_proof) {
            return false;
        }
        
        // Verify root matches checkpoint
        data_tree.root == checkpoint.training_data_root
    }
    
    /// Verify exclusion proof
    pub fn verify_exclusion(&self, proof: &TrainingExclusionProof) -> bool {
        let Some(checkpoint) = self.checkpoints.get(&proof.checkpoint_id) else {
            return false;
        };
        
        let Some(data_tree) = self.data_trees.get(&proof.checkpoint_id) else {
            return false;
        };
        
        // Verify that neighbors are actually in the tree
        for neighbor in &proof.neighbor_hashes {
            if !data_tree.leaves.contains(neighbor) {
                return false;
            }
        }
        
        // Verify sample would be at claimed position
        let absence = &proof.absence_proof;
        
        // Check neighbors bracket the sample
        if let Some(left) = absence.left_neighbor {
            if left >= proof.sample_hash {
                return false;
            }
        }
        
        if let Some(right) = absence.right_neighbor {
            if right <= proof.sample_hash {
                return false;
            }
        }
        
        // Verify tree root
        data_tree.root == checkpoint.training_data_root
    }
    
    /// Find all checkpoints containing a sample
    pub fn find_checkpoints_with_sample(&self, sample_hash: &[u8; 32]) -> Vec<String> {
        self.sample_index
            .get(sample_hash)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }
    
    /// Get checkpoint lineage
    pub fn get_lineage(&self, checkpoint_id: &str) -> Vec<ModelCheckpoint> {
        let mut lineage = Vec::new();
        let mut current = checkpoint_id.to_string();
        
        while let Some(checkpoint) = self.checkpoints.get(&current) {
            lineage.push(checkpoint.clone());
            
            match &checkpoint.parent_checkpoint {
                Some(parent) => current = parent.clone(),
                None => break,
            }
        }
        
        lineage
    }
}

/// Compute model hash (simplified)
fn compute_model_hash(checkpoint_id: String, iteration: u64) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(checkpoint_id.as_bytes());
    hasher.update(&iteration.to_le_bytes());
    hasher.finalize().into()
}

/// Current timestamp
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Model provenance API
pub struct ProvenanceAPI {
    tracker: ModelProvenance,
}

impl ProvenanceAPI {
    pub fn new() -> Self {
        ProvenanceAPI {
            tracker: ModelProvenance::new(),
        }
    }
    
    /// API: Check if model was trained on data
    pub fn was_trained_on(
        &self,
        checkpoint_id: &str,
        data_hash: &[u8; 32],
    ) -> Result<bool, ProvenanceError> {
        let checkpoints = self.tracker.find_checkpoints_with_sample(data_hash);
        Ok(checkpoints.contains(&checkpoint_id.to_string()))
    }
    
    /// API: Get training proof
    pub fn get_training_proof(
        &self,
        checkpoint_id: &str,
        data_hash: &[u8; 32],
    ) -> Result<TrainingInclusionProof, ProvenanceError> {
        self.tracker.prove_inclusion(checkpoint_id, data_hash)
            .ok_or(ProvenanceError::ProofNotFound)
    }
    
    /// API: Get exclusion proof
    pub fn get_exclusion_proof(
        &self,
        checkpoint_id: &str,
        data_hash: &[u8; 32],
    ) -> Result<TrainingExclusionProof, ProvenanceError> {
        self.tracker.prove_exclusion(checkpoint_id, data_hash)
            .ok_or(ProvenanceError::ProofNotFound)
    }
}

/// Provenance errors
#[derive(Debug, Clone)]
pub enum ProvenanceError {
    CheckpointNotFound,
    ProofNotFound,
    InvalidProof,
}

impl std::fmt::Display for ProvenanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProvenanceError::CheckpointNotFound => write!(f, "Checkpoint not found"),
            ProvenanceError::ProofNotFound => write!(f, "Proof not found"),
            ProvenanceError::InvalidProof => write!(f, "Invalid proof"),
        }
    }
}

impl std::error::Error for ProvenanceError {}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_merkle_tree() {
        let leaves: Vec<[u8; 32]> = (0..4)
            .map(|i| [i as u8; 32])
            .collect();
        
        let tree = MerkleTree::build(leaves.clone());
        
        // Verify all leaves
        for (i, leaf) in leaves.iter().enumerate() {
            let proof = tree.prove_inclusion(i).unwrap();
            assert!(tree.verify_proof(leaf, i, &proof));
        }
    }
    
    #[test]
    fn test_exclusion_proof() {
        let leaves: Vec<[u8; 32]> = (0..4)
            .map(|i| [(i * 2) as u8; 32]) // 0, 2, 4, 6
            .collect();
        
        let tree = MerkleTree::build(leaves);
        
        // Prove exclusion of 3 (between 2 and 4)
        let sample = [3u8; 32];
        let proof = tree.prove_exclusion(&sample).unwrap();
        
        assert_eq!(proof.insertion_index, 2);
        assert_eq!(proof.left_neighbor, Some([2u8; 32]));
        assert_eq!(proof.right_neighbor, Some([4u8; 32]));
    }
    
    #[test]
    fn test_model_provenance() {
        let mut tracker = ModelProvenance::new();
        
        // Create training batches
        let batch = TrainingBatch {
            batch_id: "batch1".to_string(),
            data_samples: vec![
                DataSample {
                    sample_hash: [1u8; 32],
                    category: "usage".to_string(),
                    consent_ids: vec!["consent:1".to_string()],
                    added_at: 0,
                },
                DataSample {
                    sample_hash: [2u8; 32],
                    category: "usage".to_string(),
                    consent_ids: vec!["consent:2".to_string()],
                    added_at: 0,
                },
            ],
            consent_ids: vec!["consent:1".to_string(), "consent:2".to_string()],
            batch_root: [0u8; 32],
            gradient_hash: [0u8; 32],
        };
        
        // Record checkpoint
        let checkpoint = tracker.record_checkpoint(
            "cp1".to_string(),
            "1.0".to_string(),
            1,
            &[batch],
            None,
        );
        
        // Prove inclusion
        let proof = tracker.prove_inclusion("cp1", &[1u8; 32]);
        assert!(proof.is_some());
        
        // Verify inclusion
        assert!(tracker.verify_inclusion(&proof.unwrap()));
        
        // Prove exclusion
        let exclusion = tracker.prove_exclusion("cp1", &[99u8; 32]);
        assert!(exclusion.is_some());
    }
}
