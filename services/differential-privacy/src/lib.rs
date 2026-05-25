//! Differential Privacy with Consent-Aware Noise
//! 
//! Implements (ε, δ)-differential privacy with per-user privacy budgets
//! that are tracked and enforced based on consent scope.

use rand::distributions::{Distribution, Laplace};
use rand::rngs::OsRng;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Privacy parameters (ε, δ)
#[derive(Clone, Copy, Debug)]
pub struct PrivacyParams {
    /// Epsilon: privacy budget (smaller = more private)
    pub epsilon: f64,
    /// Delta: probability of privacy breach
    pub delta: f64,
}

impl PrivacyParams {
    /// Strict privacy (ε = 0.1)
    pub fn strict() -> Self {
        PrivacyParams {
            epsilon: 0.1,
            delta: 1e-6,
        }
    }
    
    /// Moderate privacy (ε = 1.0)
    pub fn moderate() -> Self {
        PrivacyParams {
            epsilon: 1.0,
            delta: 1e-5,
        }
    }
    
    /// Relaxed privacy (ε = 10.0)
    pub fn relaxed() -> Self {
        PrivacyParams {
            epsilon: 10.0,
            delta: 1e-4,
        }
    }
}

/// Per-user privacy budget tracker
#[derive(Clone, Debug)]
pub struct PrivacyBudget {
    /// User identifier
    pub user_id: String,
    /// Total epsilon budget
    pub total_epsilon: f64,
    /// Remaining epsilon budget
    pub remaining_epsilon: f64,
    /// Queries performed
    pub queries: Vec<QueryRecord>,
    /// Consent-based budget allocation
    pub consent_allocations: HashMap<String, f64>,
}

/// Record of a privacy-preserving query
#[derive(Clone, Debug)]
pub struct QueryRecord {
    pub timestamp: u64,
    pub epsilon_used: f64,
    pub query_type: String,
    pub consent_id: String,
}

/// Consent-aware privacy budget manager
pub struct PrivacyBudgetManager {
    /// User budgets
    budgets: Arc<Mutex<HashMap<String, PrivacyBudget>>>,
    /// Default privacy parameters
    default_params: PrivacyParams,
    /// Consent-based privacy multipliers
    consent_multipliers: HashMap<String, f64>,
}

/// Differentially private query result
#[derive(Clone, Debug)]
pub struct DPResult<T> {
    /// The noisy result
    pub value: T,
    /// Privacy parameters used
    pub params: PrivacyParams,
    /// Epsilon consumed
    pub epsilon_used: f64,
    /// Remaining budget
    pub remaining_budget: f64,
}

/// Noise mechanism for differential privacy
pub enum NoiseMechanism {
    /// Laplace mechanism (for numeric queries)
    Laplace,
    /// Gaussian mechanism (for high-dimensional data)
    Gaussian,
    /// Exponential mechanism (for non-numeric outputs)
    Exponential,
}

impl PrivacyBudgetManager {
    /// Create new privacy budget manager
    pub fn new(default_params: PrivacyParams) -> Self {
        let mut consent_multipliers = HashMap::new();
        
        // Different consent purposes have different privacy needs
        consent_multipliers.insert("analytics".to_string(), 1.0);
        consent_multipliers.insert("research".to_string(), 0.5);  // Stricter
        consent_multipliers.insert("marketing".to_string(), 2.0); // More relaxed
        consent_multipliers.insert("model_training".to_string(), 1.5);
        
        PrivacyBudgetManager {
            budgets: Arc::new(Mutex::new(HashMap::new())),
            default_params,
            consent_multipliers,
        }
    }
    
    /// Initialize privacy budget for a user
    pub fn initialize_user(&self, user_id: &str, total_epsilon: f64) {
        let budget = PrivacyBudget {
            user_id: user_id.to_string(),
            total_epsilon,
            remaining_epsilon: total_epsilon,
            queries: Vec::new(),
            consent_allocations: HashMap::new(),
        };
        
        let mut budgets = self.budgets.lock().unwrap();
        budgets.insert(user_id.to_string(), budget);
    }
    
    /// Allocate budget based on consent
    /// 
    /// Different consent purposes get different privacy budgets
    pub fn allocate_consent_budget(
        &self,
        user_id: &str,
        consent_id: &str,
        purpose: &str,
    ) -> Result<f64, PrivacyError> {
        let mut budgets = self.budgets.lock().unwrap();
        let budget = budgets.get_mut(user_id)
            .ok_or(PrivacyError::UserNotFound)?;
        
        // Get multiplier for this purpose
        let multiplier = self.consent_multipliers.get(purpose)
            .copied()
            .unwrap_or(1.0);
        
        // Allocate portion of total budget
        let allocation = budget.total_epsilon * 0.2 * multiplier; // 20% per consent
        
        if allocation > budget.remaining_epsilon {
            return Err(PrivacyError::InsufficientBudget);
        }
        
        budget.consent_allocations.insert(consent_id.to_string(), allocation);
        
        Ok(allocation)
    }
    
    /// Check if query is allowed and consume budget
    fn consume_budget(
        &self,
        user_id: &str,
        epsilon: f64,
        consent_id: &str,
    ) -> Result<f64, PrivacyError> {
        let mut budgets = self.budgets.lock().unwrap();
        let budget = budgets.get_mut(user_id)
            .ok_or(PrivacyError::UserNotFound)?;
        
        // Check consent-specific budget
        let consent_budget = budget.consent_allocations.get(consent_id)
            .copied()
            .unwrap_or(budget.remaining_epsilon);
        
        if epsilon > consent_budget.min(budget.remaining_epsilon) {
            return Err(PrivacyError::InsufficientBudget);
        }
        
        // Consume budget
        budget.remaining_epsilon -= epsilon;
        
        // Record query
        budget.queries.push(QueryRecord {
            timestamp: current_timestamp(),
            epsilon_used: epsilon,
            query_type: "query".to_string(),
            consent_id: consent_id.to_string(),
        });
        
        Ok(budget.remaining_epsilon)
    }
    
    /// Execute differentially private count query
    pub fn dp_count(
        &self,
        user_id: &str,
        consent_id: &str,
        true_count: i64,
        sensitivity: f64,
    ) -> Result<DPResult<i64>, PrivacyError> {
        let epsilon = self.default_params.epsilon;
        
        // Check and consume budget
        let remaining = self.consume_budget(user_id, epsilon, consent_id)?;
        
        // Add Laplace noise
        let scale = sensitivity / epsilon;
        let noise = sample_laplace(0.0, scale);
        
        let noisy_count = (true_count as f64 + noise).round() as i64;
        
        Ok(DPResult {
            value: noisy_count.max(0),
            params: self.default_params,
            epsilon_used: epsilon,
            remaining_budget: remaining,
        })
    }
    
    /// Execute differentially private sum query
    pub fn dp_sum(
        &self,
        user_id: &str,
        consent_id: &str,
        true_sum: f64,
        sensitivity: f64,
    ) -> Result<DPResult<f64>, PrivacyError> {
        let epsilon = self.default_params.epsilon;
        
        let remaining = self.consume_budget(user_id, epsilon, consent_id)?;
        
        let scale = sensitivity / epsilon;
        let noise = sample_laplace(0.0, scale);
        
        Ok(DPResult {
            value: true_sum + noise,
            params: self.default_params,
            epsilon_used: epsilon,
            remaining_budget: remaining,
        })
    }
    
    /// Execute differentially private mean query
    pub fn dp_mean(
        &self,
        user_id: &str,
        consent_id: &str,
        values: &[f64],
        lower_bound: f64,
        upper_bound: f64,
    ) -> Result<DPResult<f64>, PrivacyError> {
        if values.is_empty() {
            return Err(PrivacyError::EmptyDataset);
        }
        
        // Clip values to bounds
        let clipped: Vec<f64> = values.iter()
            .map(|&v| v.clamp(lower_bound, upper_bound))
            .collect();
        
        let true_sum: f64 = clipped.iter().sum();
        let true_count = clipped.len() as f64;
        
        // Sensitivity for mean is (upper - lower) / n
        let sensitivity = (upper_bound - lower_bound) / true_count;
        
        let epsilon = self.default_params.epsilon;
        let remaining = self.consume_budget(user_id, epsilon, consent_id)?;
        
        let scale = sensitivity / epsilon;
        let noise = sample_laplace(0.0, scale);
        
        let noisy_mean = (true_sum / true_count) + noise;
        
        Ok(DPResult {
            value: noisy_mean.clamp(lower_bound, upper_bound),
            params: self.default_params,
            epsilon_used: epsilon,
            remaining_budget: remaining,
        })
    }
    
    /// Execute differentially private histogram
    pub fn dp_histogram(
        &self,
        user_id: &str,
        consent_id: &str,
        values: &[f64],
        bins: &[f64],
        lower_bound: f64,
        upper_bound: f64,
    ) -> Result<DPResult<Vec<i64>>, PrivacyError> {
        if bins.len() < 2 {
            return Err(PrivacyError::InvalidBins);
        }
        
        // Clip values
        let clipped: Vec<f64> = values.iter()
            .map(|&v| v.clamp(lower_bound, upper_bound))
            .collect();
        
        // Build histogram
        let mut counts = vec![0i64; bins.len() - 1];
        for &value in &clipped {
            for i in 0..bins.len() - 1 {
                if value >= bins[i] && value < bins[i + 1] {
                    counts[i] += 1;
                    break;
                }
            }
        }
        
        // Add noise to each bin (composition)
        let epsilon_per_bin = self.default_params.epsilon / bins.len() as f64;
        let remaining = self.consume_budget(user_id, self.default_params.epsilon, consent_id)?;
        
        let scale = 1.0 / epsilon_per_bin; // Sensitivity = 1 for count
        let noisy_counts: Vec<i64> = counts.iter()
            .map(|&c| (c as f64 + sample_laplace(0.0, scale)).round() as i64)
            .map(|c| c.max(0))
            .collect();
        
        Ok(DPResult {
            value: noisy_counts,
            params: PrivacyParams {
                epsilon: self.default_params.epsilon,
                delta: self.default_params.delta,
            },
            epsilon_used: self.default_params.epsilon,
            remaining_budget: remaining,
        })
    }
    
    /// Get user's privacy budget status
    pub fn get_budget_status(&self, user_id: &str) -> Result<PrivacyBudget, PrivacyError> {
        let budgets = self.budgets.lock().unwrap();
        budgets.get(user_id)
            .cloned()
            .ok_or(PrivacyError::UserNotFound)
    }
    
    /// Reset budget (e.g., after time period)
    pub fn reset_budget(&self, user_id: &str) -> Result<(), PrivacyError> {
        let mut budgets = self.budgets.lock().unwrap();
        let budget = budgets.get_mut(user_id)
            .ok_or(PrivacyError::UserNotFound)?;
        
        budget.remaining_epsilon = budget.total_epsilon;
        budget.queries.clear();
        budget.consent_allocations.clear();
        
        Ok(())
    }
}

/// Sample from Laplace distribution
fn sample_laplace(mean: f64, scale: f64) -> f64 {
    let u: f64 = rand::random();
    let sign = if u < 0.5 { -1.0 } else { 1.0 };
    mean + sign * scale * (1.0 - 2.0 * u.abs()).ln()
}

/// Sample from Gaussian distribution
fn sample_gaussian(mean: f64, std_dev: f64) -> f64 {
    use rand_distr::{Normal, Distribution};
    let normal = Normal::new(mean, std_dev).unwrap();
    normal.sample(&mut OsRng)
}

/// Current timestamp
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Privacy errors
#[derive(Debug, Clone)]
pub enum PrivacyError {
    UserNotFound,
    InsufficientBudget,
    EmptyDataset,
    InvalidBins,
    QueryFailed(String),
}

impl std::fmt::Display for PrivacyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrivacyError::UserNotFound => write!(f, "User not found"),
            PrivacyError::InsufficientBudget => write!(f, "Insufficient privacy budget"),
            PrivacyError::EmptyDataset => write!(f, "Empty dataset"),
            PrivacyError::InvalidBins => write!(f, "Invalid histogram bins"),
            PrivacyError::QueryFailed(e) => write!(f, "Query failed: {}", e),
        }
    }
}

impl std::error::Error for PrivacyError {}

/// Privacy-preserving analytics engine
pub struct PrivacyPreservingAnalytics {
    budget_manager: PrivacyBudgetManager,
}

impl PrivacyPreservingAnalytics {
    pub fn new(budget_manager: PrivacyBudgetManager) -> Self {
        PrivacyPreservingAnalytics { budget_manager }
    }
    
    /// Generate privacy-preserving report
    pub fn generate_report(
        &self,
        user_id: &str,
        consent_id: &str,
        data: &[f64],
    ) -> Result<PrivacyReport, PrivacyError> {
        let count = self.budget_manager.dp_count(
            user_id, consent_id, data.len() as i64, 1.0
        )?;
        
        let sum = self.budget_manager.dp_sum(
            user_id, consent_id, data.iter().sum(), 100.0
        )?;
        
        let mean = self.budget_manager.dp_mean(
            user_id, consent_id, data, 0.0, 1000.0
        )?;
        
        Ok(PrivacyReport {
            count: count.value,
            sum: sum.value,
            mean: mean.value,
            total_epsilon_used: count.epsilon_used + sum.epsilon_used + mean.epsilon_used,
            remaining_budget: mean.remaining_budget,
        })
    }
}

/// Privacy report
#[derive(Clone, Debug)]
pub struct PrivacyReport {
    pub count: i64,
    pub sum: f64,
    pub mean: f64,
    pub total_epsilon_used: f64,
    pub remaining_budget: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_privacy_budget_initialization() {
        let manager = PrivacyBudgetManager::new(PrivacyParams::moderate());
        manager.initialize_user("user1", 10.0);
        
        let status = manager.get_budget_status("user1").unwrap();
        assert_eq!(status.total_epsilon, 10.0);
        assert_eq!(status.remaining_epsilon, 10.0);
    }
    
    #[test]
    fn test_dp_count() {
        let manager = PrivacyBudgetManager::new(PrivacyParams::moderate());
        manager.initialize_user("user1", 10.0);
        manager.allocate_consent_budget("user1", "consent1", "analytics").unwrap();
        
        let result = manager.dp_count("user1", "consent1", 100, 1.0).unwrap();
        
        // Result should be close to 100 (with noise)
        assert!(result.value >= 80 && result.value <= 120);
        assert!(result.epsilon_used > 0.0);
        assert!(result.remaining_budget < 10.0);
    }
    
    #[test]
    fn test_budget_exhaustion() {
        let manager = PrivacyBudgetManager::new(PrivacyParams::strict());
        manager.initialize_user("user1", 0.5); // Very small budget
        manager.allocate_consent_budget("user1", "consent1", "analytics").unwrap();
        
        // First query should work
        let _ = manager.dp_count("user1", "consent1", 100, 1.0).unwrap();
        
        // Second query should fail (budget exhausted)
        let result = manager.dp_count("user1", "consent1", 100, 1.0);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_dp_mean() {
        let manager = PrivacyBudgetManager::new(PrivacyParams::moderate());
        manager.initialize_user("user1", 10.0);
        manager.allocate_consent_budget("user1", "consent1", "analytics").unwrap();
        
        let data: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let result = manager.dp_mean("user1", "consent1", &data, 0.0, 100.0).unwrap();
        
        // True mean is 49.5, result should be close
        assert!(result.value >= 40.0 && result.value <= 60.0);
    }
}
