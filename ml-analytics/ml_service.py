#!/usr/bin/env python3
"""
ML Prediction Service for HSK Platform
Serves real-time anomaly detection and risk scoring predictions
"""

import os
import json
import logging
from datetime import datetime
from typing import Dict, List, Optional
from fastapi import FastAPI, HTTPException, BackgroundTasks
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
import uvicorn
import joblib
import numpy as np
import redis
from kafka import KafkaConsumer, KafkaProducer
import threading
import time

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

app = FastAPI(title="HSK ML Service", version="1.0.0")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Global model store
models = {}
redis_client = None
kafka_producer = None


# ==================== Pydantic Models ====================

class ConsentFeatures(BaseModel):
    retention_days: float
    hour_of_day: int
    day_of_week: int
    is_weekend: int
    is_business_hours: int
    purpose_encoded: int
    legal_basis_encoded: int
    revocation_count: int


class SystemMetrics(BaseModel):
    uptime_percentage: float
    error_rate_24h: float
    avg_response_time: float
    cpu_utilization: float
    memory_utilization: float
    disk_utilization: float
    recent_changes: int


class RiskFeatures(BaseModel):
    sensitive_data: bool = False
    broad_scope: bool = False
    long_retention: int = 365
    privileged_access: bool = False
    unusual_time: bool = False
    new_device: bool = False
    known_vulnerabilities: int = 0
    outdated_components: bool = False
    gdpr_jurisdiction: bool = False
    audit_findings: int = 0


class AnomalyResponse(BaseModel):
    is_anomaly: bool
    anomaly_score: float
    confidence: float
    timestamp: str


class ViolationPredictionResponse(BaseModel):
    violation_predicted: bool
    probability: float
    risk_level: str
    timestamp: str


class RiskScoreResponse(BaseModel):
    total_score: float
    consent_risk: float
    access_risk: float
    system_risk: float
    compliance_risk: float
    risk_level: str
    timestamp: str


# ==================== Model Loading ====================

def load_models(model_dir: str = "/models"):
    """Load all trained models"""
    global models
    
    logger.info(f"Loading models from {model_dir}...")
    
    try:
        # Load consent anomaly model
        consent_data = joblib.load(f"{model_dir}/consent_anomaly_model.pkl")
        models['consent_anomaly'] = consent_data
        logger.info("Consent anomaly model loaded")
    except Exception as e:
        logger.error(f"Failed to load consent anomaly model: {e}")
    
    try:
        # Load violation prediction model
        violation_data = joblib.load(f"{model_dir}/violation_prediction_model.pkl")
        models['violation_prediction'] = violation_data
        logger.info("Violation prediction model loaded")
    except Exception as e:
        logger.warning(f"Violation prediction model not found: {e}")
    
    logger.info(f"Loaded {len(models)} models")


# ==================== API Endpoints ====================

@app.get("/health")
async def health_check():
    """Health check endpoint"""
    return {
        "status": "healthy",
        "models_loaded": len(models),
        "timestamp": datetime.now().isoformat()
    }


@app.post("/predict/consent-anomaly", response_model=AnomalyResponse)
async def predict_consent_anomaly(features: ConsentFeatures):
    """Predict if a consent pattern is anomalous"""
    if 'consent_anomaly' not in models:
        raise HTTPException(status_code=503, detail="Consent anomaly model not loaded")
    
    model_data = models['consent_anomaly']
    model = model_data['model']
    scaler = model_data['scaler']
    feature_columns = model_data['feature_columns']
    
    # Prepare features
    X = np.array([[getattr(features, col) for col in feature_columns]])
    X_scaled = scaler.transform(X)
    
    # Predict
    prediction = model.predict(X_scaled)[0]
    score = model.decision_function(X_scaled)[0]
    
    result = {
        'is_anomaly': prediction == -1,
        'anomaly_score': float(score),
        'confidence': float(1 / (1 + np.exp(-score))),
        'timestamp': datetime.now().isoformat()
    }
    
    # Cache result in Redis
    if redis_client:
        redis_client.setex(
            f"anomaly:{hash(str(features))}",
            300,
            json.dumps(result)
        )
    
    # Send to Kafka for analytics
    if kafka_producer:
        kafka_producer.send('ml-predictions', {
            'type': 'consent_anomaly',
            'features': features.dict(),
            'prediction': result
        })
    
    return result


@app.post("/predict/violation", response_model=ViolationPredictionResponse)
async def predict_violation(metrics: SystemMetrics):
    """Predict probability of violation"""
    if 'violation_prediction' not in models:
        raise HTTPException(status_code=503, detail="Violation prediction model not loaded")
    
    model_data = models['violation_prediction']
    model = model_data['model']
    scaler = model_data['scaler']
    feature_columns = model_data['feature_columns']
    
    # Prepare features
    X = np.array([[getattr(metrics, col) for col in feature_columns]])
    X_scaled = scaler.transform(X)
    
    # Predict
    proba = model.predict_proba(X_scaled)[0]
    prediction = model.predict(X_scaled)[0]
    
    probability = float(proba[1])
    
    result = {
        'violation_predicted': bool(prediction),
        'probability': probability,
        'risk_level': 'high' if probability > 0.7 else 'medium' if probability > 0.3 else 'low',
        'timestamp': datetime.now().isoformat()
    }
    
    # Alert if high risk
    if probability > 0.7 and kafka_producer:
        kafka_producer.send('ml-alerts', {
            'severity': 'high',
            'type': 'violation_predicted',
            'probability': probability,
            'metrics': metrics.dict()
        })
    
    return result


@app.post("/predict/risk-score", response_model=RiskScoreResponse)
async def calculate_risk_score(features: RiskFeatures):
    """Calculate comprehensive risk score"""
    
    # Consent risk factors (30%)
    consent_risk = 0.0
    if features.sensitive_data:
        consent_risk += 30
    if features.broad_scope:
        consent_risk += 20
    if features.long_retention > 365 * 2:
        consent_risk += 20
    consent_risk = min(100, consent_risk)
    
    # Access risk factors (30%)
    access_risk = 0.0
    if features.privileged_access:
        access_risk += 30
    if features.unusual_time:
        access_risk += 20
    if features.new_device:
        access_risk += 10
    access_risk = min(100, access_risk)
    
    # System risk factors (20%)
    system_risk = 0.0
    system_risk += min(20, features.known_vulnerabilities * 5)
    if features.outdated_components:
        system_risk += 15
    system_risk = min(100, system_risk)
    
    # Compliance risk factors (20%)
    compliance_risk = 0.0
    if features.gdpr_jurisdiction:
        compliance_risk += 10
    compliance_risk += min(20, features.audit_findings * 10)
    compliance_risk = min(100, compliance_risk)
    
    # Weighted total
    total_score = (
        consent_risk * 0.3 +
        access_risk * 0.3 +
        system_risk * 0.2 +
        compliance_risk * 0.2
    )
    
    return {
        'total_score': round(total_score, 2),
        'consent_risk': round(consent_risk, 2),
        'access_risk': round(access_risk, 2),
        'system_risk': round(system_risk, 2),
        'compliance_risk': round(compliance_risk, 2),
        'risk_level': 'critical' if total_score > 80 else 'high' if total_score > 60 
                     else 'medium' if total_score > 30 else 'low',
        'timestamp': datetime.now().isoformat()
    }


@app.get("/models")
async def list_models():
    """List loaded models"""
    return {
        'models': list(models.keys()),
        'count': len(models)
    }


@app.post("/models/reload")
async def reload_models():
    """Reload all models"""
    load_models()
    return {'status': 'success', 'models_loaded': len(models)}


# ==================== Background Tasks ====================

def kafka_consumer_loop():
    """Consume events from Kafka for real-time scoring"""
    try:
        consumer = KafkaConsumer(
            'consent-events',
            'access-logs',
            bootstrap_servers=os.getenv('KAFKA_BROKERS', 'localhost:9092'),
            value_deserializer=lambda m: json.loads(m.decode('utf-8')),
            group_id='ml-service'
        )
        
        logger.info("Kafka consumer started")
        
        for message in consumer:
            try:
                event = message.value
                topic = message.topic
                
                if topic == 'consent-events':
                    # Score consent in real-time
                    features = ConsentFeatures(
                        retention_days=event.get('retention_days', 365),
                        hour_of_day=datetime.now().hour,
                        day_of_week=datetime.now().weekday(),
                        is_weekend=1 if datetime.now().weekday() >= 5 else 0,
                        is_business_hours=1 if 9 <= datetime.now().hour <= 17 else 0,
                        purpose_encoded=event.get('purpose_encoded', 0),
                        legal_basis_encoded=event.get('legal_basis_encoded', 0),
                        revocation_count=event.get('revocation_count', 0)
                    )
                    
                    # Would call prediction here
                    logger.info(f"Scored consent event: {event.get('id')}")
                    
            except Exception as e:
                logger.error(f"Error processing Kafka message: {e}")
                
    except Exception as e:
        logger.error(f"Kafka consumer error: {e}")


def metrics_collection_loop():
    """Collect metrics for model monitoring"""
    while True:
        try:
            # Collect prediction metrics
            if redis_client:
                prediction_count = redis_client.get('prediction_count') or 0
                redis_client.set('prediction_count', int(prediction_count) + 1)
            
            time.sleep(60)  # Every minute
        except Exception as e:
            logger.error(f"Metrics collection error: {e}")
            time.sleep(60)


# ==================== Startup ====================

@app.on_event("startup")
async def startup_event():
    """Initialize on startup"""
    global redis_client, kafka_producer
    
    # Load models
    load_models()
    
    # Connect to Redis
    try:
        redis_client = redis.Redis(
            host=os.getenv('REDIS_HOST', 'localhost'),
            port=int(os.getenv('REDIS_PORT', 6379)),
            decode_responses=True
        )
        redis_client.ping()
        logger.info("Connected to Redis")
    except Exception as e:
        logger.warning(f"Redis not available: {e}")
    
    # Connect to Kafka
    try:
        kafka_producer = KafkaProducer(
            bootstrap_servers=os.getenv('KAFKA_BROKERS', 'localhost:9092'),
            value_serializer=lambda v: json.dumps(v).encode('utf-8')
        )
        logger.info("Connected to Kafka")
    except Exception as e:
        logger.warning(f"Kafka not available: {e}")
    
    # Start background threads
    threading.Thread(target=kafka_consumer_loop, daemon=True).start()
    threading.Thread(target=metrics_collection_loop, daemon=True).start()


if __name__ == '__main__':
    uvicorn.run(app, host="0.0.0.0", port=8080)
