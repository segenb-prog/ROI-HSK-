#!/usr/bin/env python3
"""
Webhook Service Implementation for HSK Platform
Handles webhook delivery, retries, and dead letter queue
"""

import os
import json
import hmac
import hashlib
import logging
import asyncio
import aiohttp
from datetime import datetime, timedelta
from typing import Dict, List, Optional, Any
from dataclasses import dataclass, asdict
from enum import Enum
import redis.asyncio as redis
import asyncpg
from kafka import KafkaConsumer, KafkaProducer
import threading
import time

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)


class WebhookStatus(Enum):
    PENDING = "pending"
    DELIVERED = "delivered"
    FAILED = "failed"
    RETRYING = "retrying"
    DLQ = "dlq"


class CircuitBreakerState(Enum):
    CLOSED = "closed"
    OPEN = "open"
    HALF_OPEN = "half_open"


@dataclass
class WebhookEndpoint:
    id: str
    url: str
    events: List[str]
    secret: str
    description: Optional[str] = None
    created_at: Optional[datetime] = None
    max_retries: int = 10
    retry_interval: int = 60
    circuit_breaker_threshold: int = 5
    circuit_breaker_timeout: int = 300


@dataclass
class WebhookEvent:
    id: str
    event_type: str
    data: Dict[str, Any]
    created_at: datetime
    endpoint_id: str
    status: WebhookStatus = WebhookStatus.PENDING
    attempt_count: int = 0
    last_attempt: Optional[datetime] = None
    error_message: Optional[str] = None


@dataclass
class WebhookDelivery:
    event_id: str
    endpoint_id: str
    status: WebhookStatus
    http_status: Optional[int] = None
    response_body: Optional[str] = None
    delivered_at: Optional[datetime] = None
    error: Optional[str] = None


class CircuitBreaker:
    """Circuit breaker pattern for webhook delivery"""
    
    def __init__(self, threshold: int = 5, timeout: int = 300):
        self.threshold = threshold
        self.timeout = timeout
        self.failure_count = 0
        self.last_failure_time = None
        self.state = CircuitBreakerState.CLOSED
        self._lock = asyncio.Lock()
    
    async def record_success(self):
        async with self._lock:
            self.failure_count = 0
            if self.state == CircuitBreakerState.HALF_OPEN:
                self.state = CircuitBreakerState.CLOSED
                logger.info("Circuit breaker closed")
    
    async def record_failure(self) -> bool:
        async with self._lock:
            self.failure_count += 1
            self.last_failure_time = datetime.now()
            
            if self.failure_count >= self.threshold:
                self.state = CircuitBreakerState.OPEN
                logger.warning(f"Circuit breaker opened after {self.failure_count} failures")
                return True
            return False
    
    async def can_execute(self) -> bool:
        async with self._lock:
            if self.state == CircuitBreakerState.CLOSED:
                return True
            
            if self.state == CircuitBreakerState.OPEN:
                if self.last_failure_time:
                    elapsed = (datetime.now() - self.last_failure_time).total_seconds()
                    if elapsed >= self.timeout:
                        self.state = CircuitBreakerState.HALF_OPEN
                        logger.info("Circuit breaker entering half-open state")
                        return True
                return False
            
            return True


class WebhookService:
    """Main webhook service"""
    
    def __init__(self):
        self.redis_client = None
        self.db_pool = None
        self.kafka_producer = None
        self.circuit_breakers: Dict[str, CircuitBreaker] = {}
        self.endpoints: Dict[str, WebhookEndpoint] = {}
        
    async def initialize(self):
        """Initialize connections"""
        # Redis
        self.redis_client = redis.Redis(
            host=os.getenv('REDIS_HOST', 'localhost'),
            port=int(os.getenv('REDIS_PORT', 6379)),
            decode_responses=True
        )
        
        # PostgreSQL
        self.db_pool = await asyncpg.create_pool(
            host=os.getenv('DB_HOST', 'localhost'),
            database=os.getenv('DB_NAME', 'consent_ledger'),
            user=os.getenv('DB_USER', 'postgres'),
            password=os.getenv('DB_PASSWORD', 'password'),
            min_size=5,
            max_size=20
        )
        
        # Kafka
        self.kafka_producer = KafkaProducer(
            bootstrap_servers=os.getenv('KAFKA_BROKERS', 'localhost:9092'),
            value_serializer=lambda v: json.dumps(v, default=str).encode('utf-8')
        )
        
        # Load endpoints
        await self.load_endpoints()
        
        logger.info("Webhook service initialized")
    
    async def load_endpoints(self):
        """Load webhook endpoints from database"""
        async with self.db_pool.acquire() as conn:
            rows = await conn.fetch("""
                SELECT id, url, events, secret, description, 
                       max_retries, retry_interval, 
                       circuit_breaker_threshold, circuit_breaker_timeout
                FROM webhook_endpoints
                WHERE active = true
            """)
            
            for row in rows:
                endpoint = WebhookEndpoint(
                    id=row['id'],
                    url=row['url'],
                    events=json.loads(row['events']),
                    secret=row['secret'],
                    description=row['description'],
                    max_retries=row['max_retries'],
                    retry_interval=row['retry_interval'],
                    circuit_breaker_threshold=row['circuit_breaker_threshold'],
                    circuit_breaker_timeout=row['circuit_breaker_timeout']
                )
                self.endpoints[endpoint.id] = endpoint
                self.circuit_breakers[endpoint.id] = CircuitBreaker(
                    threshold=endpoint.circuit_breaker_threshold,
                    timeout=endpoint.circuit_breaker_timeout
                )
        
        logger.info(f"Loaded {len(self.endpoints)} webhook endpoints")
    
    def generate_signature(self, payload: str, secret: str) -> str:
        """Generate HMAC signature for webhook"""
        signature = hmac.new(
            secret.encode('utf-8'),
            payload.encode('utf-8'),
            hashlib.sha256
        ).hexdigest()
        return f"sha256={signature}"
    
    async def deliver_webhook(self, event: WebhookEvent, endpoint: WebhookEndpoint) -> WebhookDelivery:
        """Deliver a single webhook"""
        circuit_breaker = self.circuit_breakers.get(endpoint.id)
        
        if circuit_breaker and not await circuit_breaker.can_execute():
            logger.warning(f"Circuit breaker open for endpoint {endpoint.id}")
            return WebhookDelivery(
                event_id=event.id,
                endpoint_id=endpoint.id,
                status=WebhookStatus.FAILED,
                error="Circuit breaker open"
            )
        
        # Prepare payload
        payload = {
            'id': event.id,
            'event': event.event_type,
            'created_at': event.created_at.isoformat(),
            'data': event.data
        }
        payload_json = json.dumps(payload)
        
        # Generate signature
        signature = self.generate_signature(payload_json, endpoint.secret)
        
        headers = {
            'Content-Type': 'application/json',
            'X-Webhook-Signature': signature,
            'X-Webhook-ID': event.id,
            'X-Webhook-Event': event.event_type,
            'X-Webhook-Attempt': str(event.attempt_count + 1),
            'User-Agent': 'HSK-Webhook/1.0'
        }
        
        try:
            timeout = aiohttp.ClientTimeout(total=30)
            async with aiohttp.ClientSession(timeout=timeout) as session:
                async with session.post(
                    endpoint.url,
                    data=payload_json,
                    headers=headers
                ) as response:
                    response_body = await response.text()
                    
                    delivery = WebhookDelivery(
                        event_id=event.id,
                        endpoint_id=endpoint.id,
                        status=WebhookStatus.DELIVERED if response.status < 400 else WebhookStatus.FAILED,
                        http_status=response.status,
                        response_body=response_body[:1000],  # Limit response size
                        delivered_at=datetime.now()
                    )
                    
                    if response.status < 400:
                        if circuit_breaker:
                            await circuit_breaker.record_success()
                        logger.info(f"Webhook delivered: {event.id} -> {endpoint.url}")
                    else:
                        delivery.error = f"HTTP {response.status}"
                        if circuit_breaker:
                            await circuit_breaker.record_failure()
                    
                    return delivery
                    
        except asyncio.TimeoutError:
            logger.error(f"Webhook timeout: {event.id}")
            if circuit_breaker:
                await circuit_breaker.record_failure()
            return WebhookDelivery(
                event_id=event.id,
                endpoint_id=endpoint.id,
                status=WebhookStatus.FAILED,
                error="Timeout"
            )
        except Exception as e:
            logger.error(f"Webhook delivery error: {e}")
            if circuit_breaker:
                await circuit_breaker.record_failure()
            return WebhookDelivery(
                event_id=event.id,
                endpoint_id=endpoint.id,
                status=WebhookStatus.FAILED,
                error=str(e)
            )
    
    async def process_event(self, event: WebhookEvent):
        """Process a webhook event with retries"""
        endpoint = self.endpoints.get(event.endpoint_id)
        if not endpoint:
            logger.error(f"Endpoint not found: {event.endpoint_id}")
            return
        
        max_retries = endpoint.max_retries
        
        while event.attempt_count < max_retries:
            event.attempt_count += 1
            event.last_attempt = datetime.now()
            
            delivery = await self.deliver_webhook(event, endpoint)
            
            # Store delivery attempt
            await self.store_delivery_attempt(event, delivery)
            
            if delivery.status == WebhookStatus.DELIVERED:
                event.status = WebhookStatus.DELIVERED
                await self.update_event_status(event)
                return
            
            # Calculate retry delay with exponential backoff
            delay = endpoint.retry_interval * (2 ** (event.attempt_count - 1))
            delay = min(delay, 3600)  # Max 1 hour
            
            logger.info(f"Retrying webhook {event.id} in {delay}s (attempt {event.attempt_count})")
            await asyncio.sleep(delay)
        
        # Max retries exceeded - move to DLQ
        event.status = WebhookStatus.DLQ
        await self.move_to_dlq(event)
        await self.send_dlq_alert(event)
    
    async def store_delivery_attempt(self, event: WebhookEvent, delivery: WebhookDelivery):
        """Store delivery attempt in database"""
        async with self.db_pool.acquire() as conn:
            await conn.execute("""
                INSERT INTO webhook_deliveries 
                (event_id, endpoint_id, status, http_status, response_body, 
                 delivered_at, error, attempt_number)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            """, 
                delivery.event_id, delivery.endpoint_id, delivery.status.value,
                delivery.http_status, delivery.response_body, delivery.delivered_at,
                delivery.error, event.attempt_count
            )
    
    async def update_event_status(self, event: WebhookEvent):
        """Update event status in database"""
        async with self.db_pool.acquire() as conn:
            await conn.execute("""
                UPDATE webhook_events 
                SET status = $1, attempt_count = $2, last_attempt = $3
                WHERE id = $4
            """, event.status.value, event.attempt_count, event.last_attempt, event.id)
    
    async def move_to_dlq(self, event: WebhookEvent):
        """Move event to dead letter queue"""
        async with self.db_pool.acquire() as conn:
            await conn.execute("""
                INSERT INTO webhook_dlq 
                (event_id, event_type, data, endpoint_id, attempt_count, 
                 last_error, created_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
            """, 
                event.id, event.event_type, json.dumps(event.data),
                event.endpoint_id, event.attempt_count, 
                "Max retries exceeded", datetime.now()
            )
        
        logger.warning(f"Event moved to DLQ: {event.id}")
    
    async def send_dlq_alert(self, event: WebhookEvent):
        """Send alert when event moves to DLQ"""
        alert = {
            'alert_type': 'webhook_dlq',
            'severity': 'warning',
            'event_id': event.id,
            'endpoint_id': event.endpoint_id,
            'event_type': event.event_type,
            'attempt_count': event.attempt_count,
            'timestamp': datetime.now().isoformat()
        }
        
        self.kafka_producer.send('webhook-alerts', alert)
        logger.info(f"DLQ alert sent for event: {event.id}")
    
    async def create_event(self, event_type: str, data: Dict, endpoint_id: str) -> str:
        """Create a new webhook event"""
        event_id = f"evt_{datetime.now().timestamp()}"
        
        event = WebhookEvent(
            id=event_id,
            event_type=event_type,
            data=data,
            created_at=datetime.now(),
            endpoint_id=endpoint_id
        )
        
        # Store in database
        async with self.db_pool.acquire() as conn:
            await conn.execute("""
                INSERT INTO webhook_events 
                (id, event_type, data, endpoint_id, status, created_at)
                VALUES ($1, $2, $3, $4, $5, $6)
            """, event.id, event.event_type, json.dumps(event.data),
                event.endpoint_id, event.status.value, event.created_at)
        
        # Queue for processing
        await self.redis_client.lpush('webhook_queue', json.dumps(asdict(event), default=str))
        
        logger.info(f"Webhook event created: {event_id}")
        return event_id
    
    async def register_endpoint(self, url: str, events: List[str], 
                                description: Optional[str] = None) -> str:
        """Register a new webhook endpoint"""
        import secrets
        
        endpoint_id = f"whk_{secrets.token_hex(8)}"
        secret = secrets.token_hex(32)
        
        endpoint = WebhookEndpoint(
            id=endpoint_id,
            url=url,
            events=events,
            secret=secret,
            description=description,
            created_at=datetime.now()
        )
        
        # Store in database
        async with self.db_pool.acquire() as conn:
            await conn.execute("""
                INSERT INTO webhook_endpoints 
                (id, url, events, secret, description, active, created_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
            """, endpoint.id, endpoint.url, json.dumps(endpoint.events),
                endpoint.secret, endpoint.description, True, endpoint.created_at)
        
        self.endpoints[endpoint_id] = endpoint
        self.circuit_breakers[endpoint_id] = CircuitBreaker()
        
        logger.info(f"Webhook endpoint registered: {endpoint_id}")
        return endpoint_id
    
    async def process_queue(self):
        """Process webhook queue"""
        while True:
            try:
                # Get event from queue
                event_data = await self.redis_client.brpop('webhook_queue', timeout=1)
                if event_data:
                    _, event_json = event_data
                    event_dict = json.loads(event_json)
                    event = WebhookEvent(
                        id=event_dict['id'],
                        event_type=event_dict['event_type'],
                        data=event_dict['data'],
                        created_at=datetime.fromisoformat(event_dict['created_at']),
                        endpoint_id=event_dict['endpoint_id'],
                        status=WebhookStatus(event_dict['status']),
                        attempt_count=event_dict.get('attempt_count', 0)
                    )
                    
                    # Process event
                    asyncio.create_task(self.process_event(event))
                    
            except Exception as e:
                logger.error(f"Queue processing error: {e}")
                await asyncio.sleep(1)
    
    async def run(self):
        """Run the webhook service"""
        await self.initialize()
        
        # Start queue processor
        await self.process_queue()


# Database schema for webhooks
WEBHOOK_SCHEMA = """
-- Webhook endpoints
CREATE TABLE IF NOT EXISTS webhook_endpoints (
    id VARCHAR(64) PRIMARY KEY,
    url TEXT NOT NULL,
    events JSONB NOT NULL,
    secret TEXT NOT NULL,
    description TEXT,
    active BOOLEAN DEFAULT true,
    max_retries INTEGER DEFAULT 10,
    retry_interval INTEGER DEFAULT 60,
    circuit_breaker_threshold INTEGER DEFAULT 5,
    circuit_breaker_timeout INTEGER DEFAULT 300,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Webhook events
CREATE TABLE IF NOT EXISTS webhook_events (
    id VARCHAR(64) PRIMARY KEY,
    event_type VARCHAR(128) NOT NULL,
    data JSONB NOT NULL,
    endpoint_id VARCHAR(64) REFERENCES webhook_endpoints(id),
    status VARCHAR(32) NOT NULL,
    attempt_count INTEGER DEFAULT 0,
    last_attempt TIMESTAMP,
    created_at TIMESTAMP NOT NULL
);

-- Webhook deliveries
CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id SERIAL PRIMARY KEY,
    event_id VARCHAR(64) REFERENCES webhook_events(id),
    endpoint_id VARCHAR(64) REFERENCES webhook_endpoints(id),
    status VARCHAR(32) NOT NULL,
    http_status INTEGER,
    response_body TEXT,
    delivered_at TIMESTAMP,
    error TEXT,
    attempt_number INTEGER,
    created_at TIMESTAMP DEFAULT NOW()
);

-- Dead letter queue
CREATE TABLE IF NOT EXISTS webhook_dlq (
    id SERIAL PRIMARY KEY,
    event_id VARCHAR(64),
    event_type VARCHAR(128) NOT NULL,
    data JSONB NOT NULL,
    endpoint_id VARCHAR(64),
    attempt_count INTEGER,
    last_error TEXT,
    created_at TIMESTAMP NOT NULL,
    resolved_at TIMESTAMP,
    resolution_notes TEXT
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_webhook_events_endpoint ON webhook_events(endpoint_id);
CREATE INDEX IF NOT EXISTS idx_webhook_events_status ON webhook_events(status);
CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_event ON webhook_deliveries(event_id);
CREATE INDEX IF NOT EXISTS idx_webhook_dlq_endpoint ON webhook_dlq(endpoint_id);
"""


if __name__ == '__main__':
    service = WebhookService()
    asyncio.run(service.run())
