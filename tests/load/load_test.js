import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';
import { randomString, randomIntBetween } from 'https://jslib.k6.io/k6-utils/1.2.0/index.js';

// Custom metrics
const errorRate = new Rate('errors');
const apiLatency = new Trend('api_latency');
const consentSuccess = new Counter('consent_success');
const consentFailures = new Counter('consent_failures');

// Test configuration
export const options = {
  stages: [
    { duration: '2m', target: 100 },   // Ramp up to 100 users
    { duration: '5m', target: 100 },   // Stay at 100 users
    { duration: '2m', target: 200 },   // Ramp up to 200 users
    { duration: '5m', target: 200 },   // Stay at 200 users
    { duration: '2m', target: 500 },   // Ramp up to 500 users (stress test)
    { duration: '5m', target: 500 },   // Stay at 500 users
    { duration: '5m', target: 0 },     // Ramp down
  ],
  thresholds: {
    http_req_duration: ['p(95)<500'],  // 95% of requests under 500ms
    http_req_failed: ['rate<0.01'],    // Less than 1% errors
    errors: ['rate<0.05'],             // Custom error rate
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';
const API_KEY = __ENV.API_KEY || 'test-api-key';

const headers = {
  'Authorization': `Bearer ${API_KEY}`,
  'Content-Type': 'application/json',
};

// Helper functions
function generateDID() {
  return `did:hsk:${randomString(32)}`;
}

function generateConsentPayload(did) {
  return JSON.stringify({
    did: did,
    purpose: 'Load Test Consent',
    data_categories: ['email', 'name'],
    retention_days: 365,
    legal_basis: 'consent'
  });
}

// ==================== TEST SCENARIOS ====================

export default function() {
  group('Health Checks', () => {
    const res = http.get(`${BASE_URL}/health`);
    
    check(res, {
      'health status is 200': (r) => r.status === 200,
      'health response time < 100ms': (r) => r.timings.duration < 100,
    });
    
    apiLatency.add(res.timings.duration);
    errorRate.add(res.status !== 200);
    
    sleep(1);
  });

  group('Consent Operations', () => {
    const did = generateDID();
    
    // Grant consent
    const grantRes = http.post(
      `${BASE_URL}/consent`,
      generateConsentPayload(did),
      { headers }
    );
    
    check(grantRes, {
      'grant consent status is 200': (r) => r.status === 200,
      'grant consent has id': (r) => JSON.parse(r.body).id !== undefined,
    });
    
    apiLatency.add(grantRes.timings.duration);
    
    if (grantRes.status === 200) {
      consentSuccess.add(1);
      const consentId = JSON.parse(grantRes.body).id;
      
      // Verify consent
      const verifyRes = http.get(`${BASE_URL}/consent/verify/${consentId}`, { headers });
      check(verifyRes, {
        'verify consent status is 200': (r) => r.status === 200,
      });
      
      // Revoke consent (50% of the time)
      if (Math.random() > 0.5) {
        const revokeRes = http.post(
          `${BASE_URL}/consent/revoke`,
          JSON.stringify({ consent_id: consentId, reason: 'Load test' }),
          { headers }
        );
        check(revokeRes, {
          'revoke consent status is 200': (r) => r.status === 200,
        });
      }
    } else {
      consentFailures.add(1);
    }
    
    errorRate.add(grantRes.status !== 200);
    sleep(randomIntBetween(1, 3));
  });

  group('Challenge Flow', () => {
    const challengeRes = http.post(
      `${BASE_URL}/challenge`,
      JSON.stringify({
        system_id: `system_${randomString(8)}`,
        timeout_seconds: 300
      }),
      { headers }
    );
    
    check(challengeRes, {
      'challenge status is 200': (r) => r.status === 200,
      'challenge has id': (r) => JSON.parse(r.body).challenge_id !== undefined,
    });
    
    apiLatency.add(challengeRes.timings.duration);
    errorRate.add(challengeRes.status !== 200);
    
    sleep(randomIntBetween(2, 5));
  });

  group('Certificate Operations', () => {
    const listRes = http.get(`${BASE_URL}/certificates`, { headers });
    
    check(listRes, {
      'list certificates status is 200': (r) => r.status === 200,
    });
    
    apiLatency.add(listRes.timings.duration);
    errorRate.add(listRes.status !== 200);
    
    sleep(randomIntBetween(1, 2));
  });

  group('Transparency Log', () => {
    const logRes = http.get(`${BASE_URL}/transparency/entries?limit=10`, { headers });
    
    check(logRes, {
      'transparency log status is 200': (r) => r.status === 200,
      'transparency log returns entries': (r) => JSON.parse(r.body).entries !== undefined,
    });
    
    apiLatency.add(logRes.timings.duration);
    errorRate.add(logRes.status !== 200);
    
    sleep(randomIntBetween(1, 3));
  });
}

// ==================== SMOKE TEST ====================

export function smokeTest() {
  const res = http.get(`${BASE_URL}/health`);
  
  check(res, {
    'smoke test: status is 200': (r) => r.status === 200,
    'smoke test: response time < 50ms': (r) => r.timings.duration < 50,
  });
  
  sleep(1);
}

// ==================== STRESS TEST ====================

export function stressTest() {
  group('High Load Consent', () => {
    const did = generateDID();
    
    const res = http.post(
      `${BASE_URL}/consent`,
      generateConsentPayload(did),
      { headers }
    );
    
    check(res, {
      'stress: consent accepted': (r) => r.status === 200 || r.status === 429,
    });
    
    if (res.status === 429) {
      console.log('Rate limit hit - this is expected under high load');
    }
    
    sleep(0.1); // Minimal sleep for stress test
  });
}

// ==================== SOAK TEST ====================

export function soakTest() {
  group('Sustained Load', () => {
    const res = http.get(`${BASE_URL}/health`);
    
    check(res, {
      'soak: health check passes': (r) => r.status === 200,
    });
    
    // Occasional consent operation
    if (Math.random() > 0.9) {
      const did = generateDID();
      http.post(
        `${BASE_URL}/consent`,
        generateConsentPayload(did),
        { headers }
      );
    }
    
    sleep(5);
  });
}

// ==================== SPIKE TEST ====================

export function spikeTest() {
  group('Spike Load', () => {
    // Rapid-fire requests
    for (let i = 0; i < 10; i++) {
      http.get(`${BASE_URL}/health`, { headers });
    }
    
    sleep(0.5);
  });
}

// ==================== BREAKPOINT TEST ====================

export function breakpointTest() {
  group('Find Breaking Point', () => {
    const did = generateDID();
    
    const res = http.post(
      `${BASE_URL}/consent`,
      generateConsentPayload(did),
      { headers }
    );
    
    check(res, {
      'breakpoint: request handled': (r) => r.status !== 503,
    });
    
    // No sleep - push system to limit
  });
}
