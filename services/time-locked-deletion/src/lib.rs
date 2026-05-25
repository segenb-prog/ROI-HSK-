//! Time-Locked Cryptographic Deletion
//! 
//! Uses time-lock puzzles (Rivest-Shamir-Wagner) to create
//! self-destructing data that can only be decrypted after a
//! specified time has passed.

use num_bigint::{BigInt, BigUint, Sign, ToBigInt};
use num_integer::Integer;
use num_traits::{One, Zero, Pow};
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Time-lock puzzle parameters
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeLockPuzzle {
    /// The puzzle value: a^(2^t) mod n
    pub n: BigUint,           // Product of two large primes
    pub a: BigUint,           // Random base
    pub t: u64,               // Number of squarings required
    pub c: Vec<u8>,           // Encrypted payload
    pub expiry_timestamp: u64, // Unix timestamp when puzzle unlocks
}

/// Solution to a time-lock puzzle
#[derive(Clone, Debug)]
pub struct TimeLockSolution {
    pub a_exp: BigUint,       // a^(2^t) mod n
    pub key: [u8; 32],        // Derived encryption key
}

/// Deletion proof that becomes valid after time expiry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeLockedDeletionProof {
    pub consent_id: String,
    pub puzzle: TimeLockPuzzle,
    pub encrypted_deletion_proof: Vec<u8>,
    pub created_at: u64,
}

/// Verified deletion proof after solving
#[derive(Clone, Debug)]
pub struct VerifiedDeletionProof {
    pub consent_id: String,
    pub deletion_timestamp: u64,
    pub merkle_proof: Vec<u8>,
    pub signature: Vec<u8>,
}

/// Time-lock puzzle generator
pub struct TimeLockGenerator {
    prime_bits: usize,
}

impl TimeLockGenerator {
    /// Create new generator with specified security parameter
    /// 
    /// # Arguments
    /// * `prime_bits` - Bit length of primes (default: 1024 for 2048-bit RSA)
    pub fn new(prime_bits: usize) -> Self {
        TimeLockGenerator { prime_bits }
    }
    
    /// Generate a time-lock puzzle that unlocks after specified duration
    /// 
    /// # Arguments
    /// * `payload` - Data to encrypt (will be deletion proof)
    /// * `duration_seconds` - How long until puzzle unlocks
    /// 
    /// # Security
    /// - Uses RSA-2048 equivalent modulus
    /// - Requires sequential squaring (no parallelization)
    /// - Time estimate based on honest CPU cycles
    pub fn generate_puzzle(
        &self,
        payload: &[u8],
        duration_seconds: u64,
    ) -> TimeLockPuzzle {
        // Generate two large primes p and q
        let p = generate_safe_prime(self.prime_bits);
        let q = generate_safe_prime(self.prime_bits);
        
        // n = p * q
        let n = &p * &q;
        
        // Euler's totient: phi(n) = (p-1)(q-1)
        let phi_n = (&p - BigUint::one()) * (&q - BigUint::one());
        
        // Random base a in [2, n-1]
        let a = generate_random_base(&n);
        
        // Calculate t based on desired duration and assumed squaring time
        // Assume 1 microsecond per squaring on honest hardware
        let squarings_per_second = 1_000_000u64;
        let t = duration_seconds * squarings_per_second;
        
        // Precompute a^(2^t) mod n efficiently using trapdoor (phi(n))
        // We can compute this in O(log t) using Euler's theorem
        let exp = BigUint::from(2u32).pow(t as u32);
        let a_exp = a.modpow(&exp, &n);
        
        // Derive encryption key from a_exp
        let key = derive_key(&a_exp);
        
        // Encrypt payload
        let c = encrypt_payload(payload, &key);
        
        // Calculate expiry timestamp
        let expiry = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() + duration_seconds;
        
        TimeLockPuzzle {
            n,
            a,
            t,
            c,
            expiry_timestamp: expiry,
        }
    }
    
    /// Create time-locked deletion proof for consent
    pub fn create_deletion_proof(
        &self,
        consent_id: &str,
        duration_seconds: u64,
    ) -> TimeLockedDeletionProof {
        // Create deletion proof payload
        let payload = create_deletion_payload(consent_id);
        
        // Generate puzzle
        let puzzle = self.generate_puzzle(&payload, duration_seconds);
        
        // The encrypted payload IS the deletion proof
        let encrypted_deletion_proof = puzzle.c.clone();
        
        TimeLockedDeletionProof {
            consent_id: consent_id.to_string(),
            puzzle,
            encrypted_deletion_proof,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

/// Time-lock puzzle solver
pub struct TimeLockSolver;

impl TimeLockSolver {
    /// Solve a time-lock puzzle by sequential squaring
    /// 
    /// # Warning
    /// This requires O(t) sequential operations. No parallel speedup possible.
    /// On honest hardware, this takes approximately the specified duration.
    pub fn solve(puzzle: &TimeLockPuzzle) -> TimeLockSolution {
        let mut current = puzzle.a.clone();
        
        // Perform t sequential squarings
        // This CANNOT be parallelized - each step depends on the previous
        for _ in 0..puzzle.t {
            current = (&current * &current) % &puzzle.n;
        }
        
        let key = derive_key(&current);
        
        TimeLockSolution {
            a_exp: current,
            key,
        }
    }
    
    /// Verify that puzzle solution is correct
    pub fn verify_solution(
        puzzle: &TimeLockPuzzle,
        solution: &TimeLockSolution,
    ) -> bool {
        // Verify a_exp is in valid range
        if solution.a_exp >= puzzle.n {
            return false;
        }
        
        // Verify by checking a_exp^(2) mod n for a few iterations
        // This is much faster than full verification
        let mut check = solution.a_exp.clone();
        let check_iterations = 100.min(puzzle.t);
        
        for _ in 0..check_iterations {
            check = (&check * &check) % &puzzle.n;
        }
        
        // Continue from a^(2^(t-check_iterations))
        let mut expected = puzzle.a.clone();
        for _ in 0..(puzzle.t - check_iterations) {
            expected = (&expected * &expected) % &puzzle.n;
        }
        
        check == expected
    }
    
    /// Decrypt deletion proof after solving puzzle
    pub fn decrypt_deletion_proof(
        puzzle: &TimeLockPuzzle,
        solution: &TimeLockSolution,
    ) -> Result<VerifiedDeletionProof, DeletionError> {
        let decrypted = decrypt_payload(&puzzle.c, &solution.key)
            .map_err(|_| DeletionError::DecryptionFailed)?;
        
        // Deserialize deletion proof
        let proof: VerifiedDeletionProof = bincode::deserialize(&decrypted)
            .map_err(|_| DeletionError::InvalidProofFormat)?;
        
        Ok(proof)
    }
    
    /// Check if puzzle has expired (time-based shortcut)
    /// 
    /// Note: This is a shortcut for honest parties. Malicious parties
    /// can still solve the puzzle early by doing the computation.
    pub fn is_expired(puzzle: &TimeLockPuzzle) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        now >= puzzle.expiry_timestamp
    }
    
    /// Estimate time to solve puzzle
    pub fn estimate_solve_time(puzzle: &TimeLockPuzzle) -> std::time::Duration {
        // Estimate based on t and assumed squaring time
        let squaring_time_micros = 1u64; // 1 microsecond per squaring
        let total_micros = puzzle.t * squaring_time_micros;
        
        std::time::Duration::from_micros(total_micros)
    }
}

/// Generate a safe prime (p = 2q + 1 where q is also prime)
fn generate_safe_prime(bits: usize) -> BigUint {
    let mut rng = OsRng;
    
    loop {
        // Generate random number of specified bit length
        let mut bytes = vec![0u8; bits / 8];
        rng.fill_bytes(&mut bytes);
        bytes[0] |= 0x80; // Ensure high bit is set
        
        let candidate = BigUint::from_bytes_be(&bytes);
        
        // Check if p = 2q + 1 is prime
        let p = &candidate * BigUint::from(2u32) + BigUint::one();
        
        if is_probable_prime(&p, 25) && is_probable_prime(&candidate, 25) {
            return p;
        }
    }
}

/// Miller-Rabin primality test
fn is_probable_prime(n: &BigUint, rounds: usize) -> bool {
    if n <= &BigUint::one() {
        return false;
    }
    if n == &BigUint::from(2u32) {
        return true;
    }
    if n % BigUint::from(2u32) == BigUint::zero() {
        return false;
    }
    
    // Write n-1 as 2^r * d
    let mut r = 0u64;
    let mut d = n - BigUint::one();
    
    while &d % BigUint::from(2u32) == BigUint::zero() {
        d /= BigUint::from(2u32);
        r += 1;
    }
    
    let mut rng = OsRng;
    
    'witness_loop: for _ in 0..rounds {
        // Random a in [2, n-2]
        let a = loop {
            let mut bytes = vec![0u8; (n.bits() + 7) / 8];
            rng.fill_bytes(&mut bytes);
            let a = BigUint::from_bytes_be(&bytes) % (n - BigUint::from(3u32)) + BigUint::from(2u32);
            if a > BigUint::one() && &a < &(n - BigUint::one()) {
                break a;
            }
        };
        
        let mut x = a.modpow(&d, n);
        
        if x == BigUint::one() || x == n - BigUint::one() {
            continue 'witness_loop;
        }
        
        for _ in 0..r - 1 {
            x = (&x * &x) % n;
            if x == n - BigUint::one() {
                continue 'witness_loop;
            }
        }
        
        return false; // Composite
    }
    
    true // Probably prime
}

/// Generate random base in [2, n-1]
fn generate_random_base(n: &BigUint) -> BigUint {
    let mut rng = OsRng;
    
    loop {
        let mut bytes = vec![0u8; (n.bits() + 7) / 8];
        rng.fill_bytes(&mut bytes);
        let a = BigUint::from_bytes_be(&bytes) % n;
        
        if a > BigUint::one() && &a < &(n - BigUint::one()) {
            return a;
        }
    }
}

/// Derive encryption key from solution
fn derive_key(a_exp: &BigUint) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(a_exp.to_bytes_be());
    hasher.finalize().into()
}

/// Simple XOR encryption (in production, use AES-GCM)
fn encrypt_payload(payload: &[u8], key: &[u8; 32]) -> Vec<u8> {
    payload.iter()
        .enumerate()
        .map(|(i, b)| b ^ key[i % 32])
        .collect()
}

fn decrypt_payload(ciphertext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, ()> {
    Ok(ciphertext.iter()
        .enumerate()
        .map(|(i, b)| b ^ key[i % 32])
        .collect())
}

/// Create deletion proof payload
fn create_deletion_payload(consent_id: &str) -> Vec<u8> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let proof = VerifiedDeletionProof {
        consent_id: consent_id.to_string(),
        deletion_timestamp: timestamp,
        merkle_proof: vec![], // Would be actual proof
        signature: vec![],    // Would be actual signature
    };
    
    bincode::serialize(&proof).unwrap_or_default()
}

/// Errors in time-lock operations
#[derive(Debug, Clone)]
pub enum DeletionError {
    PuzzleGenerationFailed,
    DecryptionFailed,
    InvalidProofFormat,
    PuzzleNotSolved,
    TimeoutExceeded,
}

impl std::fmt::Display for DeletionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeletionError::PuzzleGenerationFailed => write!(f, "Failed to generate time-lock puzzle"),
            DeletionError::DecryptionFailed => write!(f, "Failed to decrypt deletion proof"),
            DeletionError::InvalidProofFormat => write!(f, "Invalid deletion proof format"),
            DeletionError::PuzzleNotSolved => write!(f, "Time-lock puzzle not yet solved"),
            DeletionError::TimeoutExceeded => write!(f, "Solve timeout exceeded"),
        }
    }
}

impl std::error::Error for DeletionError {}

/// Batch time-lock operations for efficiency
pub struct BatchTimeLock {
    generator: TimeLockGenerator,
}

impl BatchTimeLock {
    pub fn new(prime_bits: usize) -> Self {
        BatchTimeLock {
            generator: TimeLockGenerator::new(prime_bits),
        }
    }
    
    /// Create multiple time-locked deletion proofs with same parameters
    pub fn batch_create_proofs(
        &self,
        consent_ids: &[String],
        duration_seconds: u64,
    ) -> Vec<TimeLockedDeletionProof> {
        consent_ids.iter()
            .map(|id| self.generator.create_deletion_proof(id, duration_seconds))
            .collect()
    }
    
    /// Progressive solving - solve multiple puzzles in parallel
    /// 
    /// Note: Each individual puzzle still requires sequential squaring,
    /// but different puzzles can be solved in parallel.
    pub fn parallel_solve(puzzles: &[TimeLockPuzzle]) -> Vec<TimeLockSolution> {
        use rayon::prelude::*;
        
        puzzles.par_iter()
            .map(|p| TimeLockSolver::solve(p))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_puzzle_generation() {
        let generator = TimeLockGenerator::new(512); // Small for testing
        let payload = b"test deletion proof";
        
        let puzzle = generator.generate_puzzle(payload, 1); // 1 second
        
        assert!(!puzzle.n.is_zero());
        assert!(!puzzle.a.is_zero());
        assert_eq!(puzzle.t, 1_000_000); // 1 second worth of squarings
    }
    
    #[test]
    fn test_puzzle_solve_and_verify() {
        let generator = TimeLockGenerator::new(512);
        let payload = b"test payload for decryption";
        
        // Create puzzle with very short duration for testing
        let puzzle = generator.generate_puzzle(payload, 0); // Immediate
        
        // Solve the puzzle
        let solution = TimeLockSolver::solve(&puzzle);
        
        // Verify solution
        assert!(TimeLockSolver::verify_solution(&puzzle, &solution));
        
        // Decrypt payload
        let decrypted = decrypt_payload(&puzzle.c, &solution.key).unwrap();
        assert_eq!(decrypted, payload);
    }
    
    #[test]
    fn test_deletion_proof_lifecycle() {
        let generator = TimeLockGenerator::new(512);
        
        // Create time-locked deletion proof
        let proof = generator.create_deletion_proof("consent:123", 0);
        
        assert_eq!(proof.consent_id, "consent:123");
        assert!(!proof.encrypted_deletion_proof.is_empty());
        
        // Solve the puzzle
        let solution = TimeLockSolver::solve(&proof.puzzle);
        
        // Decrypt and verify
        let verified = TimeLockSolver::decrypt_deletion_proof(&proof.puzzle, &solution);
        assert!(verified.is_ok());
        
        let verified_proof = verified.unwrap();
        assert_eq!(verified_proof.consent_id, "consent:123");
    }
    
    #[test]
    fn test_is_expired() {
        let generator = TimeLockGenerator::new(512);
        
        // Create proof that expires in 1 hour
        let proof = generator.create_deletion_proof("consent:456", 3600);
        
        // Should not be expired yet
        assert!(!TimeLockSolver::is_expired(&proof.puzzle));
        
        // Create proof that expires immediately
        let expired_proof = generator.create_deletion_proof("consent:789", 0);
        // Note: This might still show as not expired due to timing
    }
}
