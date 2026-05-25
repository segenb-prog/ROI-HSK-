#!/usr/bin/env python3
"""
ML Training Pipeline for HSK Platform
Trains anomaly detection models for consent patterns, access patterns, and fraud detection
"""

import os
import json
import logging
from datetime import datetime, timedelta
from typing import Dict, List, Tuple, Optional
import numpy as np
import pandas as pd
from sklearn.ensemble import IsolationForest, GradientBoostingClassifier, RandomForestClassifier
from sklearn.preprocessing import StandardScaler, LabelEncoder
from sklearn.model_selection import train_test_split, cross_val_score
from sklearn.metrics import classification_report, roc_auc_score, precision_recall_curve
import joblib
import psycopg2
from psycopg2.extras import RealDictCursor
import redis
import clickhouse_driver
from kafka import KafkaConsumer, KafkaProducer
import warnings
warnings.filterwarnings('ignore')

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


class DataCollector:
    """Collect training data from various sources"""
    
    def __init__(self):
        self.db_conn = psycopg2.connect(
            host=os.getenv('DB_HOST', 'localhost'),
            database=os.getenv('DB_NAME', 'consent_ledger'),
            user=os.getenv('DB_USER', 'postgres'),
            password=os.getenv('DB_PASSWORD', 'password')
        )
        self.redis_client = redis.Redis(
            host=os.getenv('REDIS_HOST', 'localhost'),
            port=int(os.getenv('REDIS_PORT', 6379)),
            decode_responses=True
        )
        self.clickhouse_client = clickhouse_driver.Client(
            host=os.getenv('CLICKHOUSE_HOST', 'localhost')
        )
    
    def collect_consent_data(self, days: int = 90) -> pd.DataFrame:
        """Collect consent-related features"""
        logger.info(f"Collecting consent data for last {days} days...")
        
        query = """
        SELECT 
            c.id,
            c.did,
            c.purpose,
            c.data_categories,
            c.legal_basis,
            c.granted_at,
            c.expires_at,
            c.status,
            c.created_at,
            EXTRACT(EPOCH FROM (c.expires_at - c.granted_at))/86400 as retention_days,
            COUNT(r.id) as revocation_count,
            CASE WHEN c.status = 'revoked' THEN 1 ELSE 0 END as was_revoked
        FROM consent_entries c
        LEFT JOIN consent_revocations r ON c.id = r.consent_id
        WHERE c.created_at >= NOW() - INTERVAL '%s days'
        GROUP BY c.id
        """
        
        df = pd.read_sql(query, self.db_conn, params=(days,))
        
        # Feature engineering
        df['hour_of_day'] = pd.to_datetime(df['granted_at']).dt.hour
        df['day_of_week'] = pd.to_datetime(df['granted_at']).dt.dayofweek
        df['is_weekend'] = df['day_of_week'].isin([5, 6]).astype(int)
        df['is_business_hours'] = df['hour_of_day'].between(9, 17).astype(int)
        
        # Encode categorical features
        le_purpose = LabelEncoder()
        df['purpose_encoded'] = le_purpose.fit_transform(df['purpose'])
        
        le_basis = LabelEncoder()
        df['legal_basis_encoded'] = le_basis.fit_transform(df['legal_basis'])
        
        return df
    
    def collect_access_logs(self, days: int = 30) -> pd.DataFrame:
        """Collect access log features"""
        logger.info(f"Collecting access logs for last {days} days...")
        
        query = """
        SELECT 
            did,
            endpoint,
            method,
            status_code,
            response_time_ms,
            user_agent,
            ip_address,
            created_at,
            COUNT(*) OVER (PARTITION BY did, DATE(created_at)) as daily_requests,
            COUNT(DISTINCT endpoint) OVER (PARTITION BY did) as unique_endpoints
        FROM access_logs
        WHERE created_at >= NOW() - INTERVAL '%s days'
        """
        
        df = pd.read_sql(query, self.db_conn, params=(days,))
        
        # Feature engineering
        df['hour'] = pd.to_datetime(df['created_at']).dt.hour
        df['is_error'] = (df['status_code'] >= 400).astype(int)
        df['response_time_bucket'] = pd.cut(df['response_time_ms'], 
                                            bins=[0, 100, 500, 1000, 5000, float('inf')],
                                            labels=['fast', 'normal', 'slow', 'very_slow', 'timeout'])
        
        return df
    
    def collect_violation_data(self, days: int = 180) -> pd.DataFrame:
        """Collect violation history for training"""
        logger.info(f"Collecting violation data for last {days} days...")
        
        query = """
        SELECT 
            v.id,
            v.system_id,
            v.violation_type,
            v.severity,
            v.created_at,
            s.uptime_percentage,
            s.error_rate_24h,
            s.avg_response_time,
            s.cpu_utilization,
            s.memory_utilization,
            s.disk_utilization,
            COUNT(c.id) as recent_changes
        FROM violations v
        JOIN system_metrics s ON v.system_id = s.system_id
        LEFT JOIN system_changes c ON v.system_id = c.system_id 
            AND c.created_at >= v.created_at - INTERVAL '7 days'
        WHERE v.created_at >= NOW() - INTERVAL '%s days'
        GROUP BY v.id, s.system_id
        """
        
        return pd.read_sql(query, self.db_conn, params=(days,))


class ConsentAnomalyModel:
    """Anomaly detection for consent patterns"""
    
    def __init__(self):
        self.model = IsolationForest(
            n_estimators=100,
            contamination=0.05,
            random_state=42,
            n_jobs=-1
        )
        self.scaler = StandardScaler()
        self.feature_columns = [
            'retention_days', 'hour_of_day', 'day_of_week',
            'is_weekend', 'is_business_hours', 'purpose_encoded',
            'legal_basis_encoded', 'revocation_count'
        ]
    
    def train(self, df: pd.DataFrame) -> Dict:
        """Train the anomaly detection model"""
        logger.info("Training consent anomaly detection model...")
        
        X = df[self.feature_columns].fillna(0)
        X_scaled = self.scaler.fit_transform(X)
        
        self.model.fit(X_scaled)
        
        # Calculate anomaly scores
        scores = self.model.decision_function(X_scaled)
        predictions = self.model.predict(X_scaled)
        
        # Model metrics
        anomaly_rate = (predictions == -1).sum() / len(predictions)
        
        metrics = {
            'model_type': 'IsolationForest',
            'n_samples': len(X),
            'n_features': len(self.feature_columns),
            'anomaly_rate': float(anomaly_rate),
            'mean_anomaly_score': float(scores.mean()),
            'feature_importance': dict(zip(self.feature_columns, 
                                          np.abs(self.model.decision_function(X_scaled)).mean(axis=0).tolist()))
        }
        
        logger.info(f"Model trained. Anomaly rate: {anomaly_rate:.2%}")
        return metrics
    
    def predict(self, features: Dict) -> Dict:
        """Predict if a consent pattern is anomalous"""
        X = np.array([[features.get(col, 0) for col in self.feature_columns]])
        X_scaled = self.scaler.transform(X)
        
        prediction = self.model.predict(X_scaled)[0]
        score = self.model.decision_function(X_scaled)[0]
        
        return {
            'is_anomaly': prediction == -1,
            'anomaly_score': float(score),
            'confidence': float(1 / (1 + np.exp(-score)))
        }
    
    def save(self, path: str):
        """Save model to disk"""
        joblib.dump({
            'model': self.model,
            'scaler': self.scaler,
            'feature_columns': self.feature_columns
        }, path)
        logger.info(f"Model saved to {path}")
    
    def load(self, path: str):
        """Load model from disk"""
        data = joblib.load(path)
        self.model = data['model']
        self.scaler = data['scaler']
        self.feature_columns = data['feature_columns']
        logger.info(f"Model loaded from {path}")


class ViolationPredictionModel:
    """Predict violations before they occur"""
    
    def __init__(self):
        self.model = GradientBoostingClassifier(
            n_estimators=200,
            learning_rate=0.1,
            max_depth=5,
            random_state=42
        )
        self.scaler = StandardScaler()
        self.feature_columns = [
            'uptime_percentage', 'error_rate_24h', 'avg_response_time',
            'cpu_utilization', 'memory_utilization', 'disk_utilization',
            'recent_changes'
        ]
    
    def train(self, df: pd.DataFrame) -> Dict:
        """Train the violation prediction model"""
        logger.info("Training violation prediction model...")
        
        X = df[self.feature_columns].fillna(0)
        y = (df['severity'].isin(['high', 'critical'])).astype(int)
        
        X_train, X_test, y_train, y_test = train_test_split(
            X, y, test_size=0.2, random_state=42, stratify=y
        )
        
        X_train_scaled = self.scaler.fit_transform(X_train)
        X_test_scaled = self.scaler.transform(X_test)
        
        self.model.fit(X_train_scaled, y_train)
        
        # Evaluate
        y_pred = self.model.predict(X_test_scaled)
        y_pred_proba = self.model.predict_proba(X_test_scaled)[:, 1]
        
        auc = roc_auc_score(y_test, y_pred_proba)
        cv_scores = cross_val_score(self.model, X_train_scaled, y_train, cv=5)
        
        metrics = {
            'model_type': 'GradientBoostingClassifier',
            'n_samples': len(X),
            'auc_score': float(auc),
            'cv_mean': float(cv_scores.mean()),
            'cv_std': float(cv_scores.std()),
            'feature_importance': dict(zip(self.feature_columns, 
                                          self.model.feature_importances_.tolist()))
        }
        
        logger.info(f"Model trained. AUC: {auc:.3f}, CV: {cv_scores.mean():.3f}")
        return metrics
    
    def predict(self, features: Dict) -> Dict:
        """Predict violation probability"""
        X = np.array([[features.get(col, 0) for col in self.feature_columns]])
        X_scaled = self.scaler.transform(X)
        
        proba = self.model.predict_proba(X_scaled)[0]
        prediction = self.model.predict(X_scaled)[0]
        
        return {
            'violation_predicted': bool(prediction),
            'probability': float(proba[1]),
            'risk_level': 'high' if proba[1] > 0.7 else 'medium' if proba[1] > 0.3 else 'low'
        }
    
    def save(self, path: str):
        """Save model to disk"""
        joblib.dump({
            'model': self.model,
            'scaler': self.scaler,
            'feature_columns': self.feature_columns
        }, path)
        logger.info(f"Model saved to {path}")
    
    def load(self, path: str):
        """Load model from disk"""
        data = joblib.load(path)
        self.model = data['model']
        self.scaler = data['scaler']
        self.feature_columns = data['feature_columns']
        logger.info(f"Model loaded from {path}")


class RiskScoringModel:
    """Calculate risk scores for consent requests"""
    
    def __init__(self):
        self.model = RandomForestClassifier(
            n_estimators=100,
            max_depth=10,
            random_state=42
        )
        self.scaler = StandardScaler()
    
    def calculate_risk_score(self, features: Dict) -> Dict:
        """Calculate comprehensive risk score"""
        
        # Consent risk factors
        consent_risk = 0.0
        if features.get('sensitive_data', False):
            consent_risk += 0.3
        if features.get('broad_scope', False):
            consent_risk += 0.2
        if features.get('long_retention', 0) > 365 * 2:
            consent_risk += 0.2
        
        # Access risk factors
        access_risk = 0.0
        if features.get('privileged_access', False):
            access_risk += 0.3
        if features.get('unusual_time', False):
            access_risk += 0.2
        if features.get('new_device', False):
            access_risk += 0.1
        
        # System risk factors
        system_risk = 0.0
        if features.get('known_vulnerabilities', 0) > 0:
            system_risk += 0.2
        if features.get('outdated_components', False):
            system_risk += 0.15
        
        # Compliance risk factors
        compliance_risk = 0.0
        if features.get('gdpr_jurisdiction', False):
            compliance_risk += 0.1
        if features.get('audit_findings', 0) > 0:
            compliance_risk += 0.2
        
        # Weighted total
        total_score = (
            consent_risk * 0.3 +
            access_risk * 0.3 +
            system_risk * 0.2 +
            compliance_risk * 0.2
        ) * 100
        
        return {
            'total_score': min(100, total_score),
            'consent_risk': consent_risk * 100,
            'access_risk': access_risk * 100,
            'system_risk': system_risk * 100,
            'compliance_risk': compliance_risk * 100,
            'risk_level': 'critical' if total_score > 80 else 'high' if total_score > 60 
                         else 'medium' if total_score > 30 else 'low'
        }


class TrainingPipeline:
    """Main training pipeline orchestrator"""
    
    def __init__(self, model_dir: str = "/models"):
        self.model_dir = model_dir
        self.data_collector = DataCollector()
        self.consent_anomaly_model = ConsentAnomalyModel()
        self.violation_prediction_model = ViolationPredictionModel()
        self.risk_scoring_model = RiskScoringModel()
        
        os.makedirs(model_dir, exist_ok=True)
    
    def run_full_training(self) -> Dict:
        """Run complete training pipeline"""
        logger.info("Starting full training pipeline...")
        
        results = {}
        
        # 1. Collect data
        consent_df = self.data_collector.collect_consent_data(days=90)
        access_df = self.data_collector.collect_access_logs(days=30)
        violation_df = self.data_collector.collect_violation_data(days=180)
        
        # 2. Train consent anomaly model
        logger.info("Training consent anomaly model...")
        consent_metrics = self.consent_anomaly_model.train(consent_df)
        self.consent_anomaly_model.save(f"{self.model_dir}/consent_anomaly_model.pkl")
        results['consent_anomaly'] = consent_metrics
        
        # 3. Train violation prediction model
        if len(violation_df) > 100:
            logger.info("Training violation prediction model...")
            violation_metrics = self.violation_prediction_model.train(violation_df)
            self.violation_prediction_model.save(f"{self.model_dir}/violation_prediction_model.pkl")
            results['violation_prediction'] = violation_metrics
        else:
            logger.warning("Insufficient violation data for training")
            results['violation_prediction'] = {'status': 'skipped', 'reason': 'insufficient_data'}
        
        # 4. Save training metadata
        metadata = {
            'training_date': datetime.now().isoformat(),
            'consent_samples': len(consent_df),
            'access_samples': len(access_df),
            'violation_samples': len(violation_df),
            'results': results
        }
        
        with open(f"{self.model_dir}/training_metadata.json", 'w') as f:
            json.dump(metadata, f, indent=2)
        
        logger.info("Training pipeline completed successfully")
        return results
    
    def load_models(self):
        """Load all trained models"""
        self.consent_anomaly_model.load(f"{self.model_dir}/consent_anomaly_model.pkl")
        if os.path.exists(f"{self.model_dir}/violation_prediction_model.pkl"):
            self.violation_prediction_model.load(f"{self.model_dir}/violation_prediction_model.pkl")
        logger.info("All models loaded")


if __name__ == '__main__':
    pipeline = TrainingPipeline()
    results = pipeline.run_full_training()
    print(json.dumps(results, indent=2))
