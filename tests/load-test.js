import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

// Custom metrics
const errorRate = new Rate('errors');
const verificationDuration = new Trend('verification_duration');

// Test configuration
export const options = {
  stages: [
    { duration: '1m', target: 10 },    // Ramp up to 10 users
    { duration: '3m', target: 50 },    // Ramp up to 50 users
    { duration: '5m', target: 100 },   // Stay at 100 users
    { duration: '2m', target: 200 },   // Ramp up to 200 users
    { duration: '5m', target: 200 },   // Stay at 200 users
    { duration: '2m', target: 0 },     // Ramp down
  ],
  thresholds: {
    http_req_duration: ['p(95)<500'],   // 95% of requests under 500ms
    http_req_failed: ['rate<0.01'],      // Error rate under 1%
    errors: ['rate<0.05'],               // Custom error rate under 5%
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';

// Generate random system ID
function randomSystemId() {
  return `system-${Math.random().toString(36).substring(7)}`;
}

// Generate random DID
function randomDID() {
  return `did:hsk:citizen:${Math.random().toString(36).substring(7)}`;
}

// Health check
export function healthCheck() {
  const response = http.get(`${BASE_URL}/health`);
  
  const success = check(response, {
    'health status is 200': (r) => r.status === 200,
    'health response is valid': (r) => r.json('status') === 'healthy',
  });
  
  errorRate.add(!success);
}

// Challenge creation
export function createChallenge() {
  const systemId = randomSystemId();
  
  const payload = JSON.stringify({
    system_id: systemId,
    timeout_hours: 72,
  });
  
  const start = Date.now();
  const response = http.post(
    `${BASE_URL}/challenge`,
    payload,
    { headers: { 'Content-Type': 'application/json' } }
  );
  const duration = Date.now() - start;
  
  verificationDuration.add(duration);
  
  const success = check(response, {
    'challenge status is 200': (r) => r.status === 200,
    'challenge has request_id': (r) => r.json('request_id') !== undefined,
  });
  
  errorRate.add(!success);
  
  if (success) {
    return response.json('request_id');
  }
  
  return null;
}

// Submit response (will fail due to missing proofs, but tests the flow)
export function submitResponse() {
  const requestId = createChallenge();
  if (!requestId) return;
  
  const payload = JSON.stringify({
    request_id: requestId,
    system_id: randomSystemId(),
    provided_proofs: [],
    submitted_at: new Date().toISOString(),
  });
  
  const start = Date.now();
  const response = http.post(
    `${BASE_URL}/response`,
    payload,
    { headers: { 'Content-Type': 'application/json' } }
  );
  const duration = Date.now() - start;
  
  verificationDuration.add(duration);
  
  const success = check(response, {
    'response status is 200': (r) => r.status === 200,
    'response has status field': (r) => r.json('status') !== undefined,
  });
  
  errorRate.add(!success);
}

// List certificates
export function listCertificates() {
  const response = http.get(`${BASE_URL}/certificates?limit=10`);
  
  const success = check(response, {
    'certificates status is 200': (r) => r.status === 200,
    'certificates is array': (r) => Array.isArray(r.json()),
  });
  
  errorRate.add(!success);
}

// Digital Identity: Register citizen
export function registerCitizen() {
  const did = randomDID();
  
  // Generate a random Ed25519 public key (32 bytes)
  const publicKey = Array.from({length: 32}, () => Math.floor(Math.random() * 256));
  const publicKeyBase64 = btoa(String.fromCharCode(...publicKey));
  
  const payload = JSON.stringify({
    did: did,
    public_key: publicKeyBase64,
  });
  
  const response = http.post(
    `${BASE_URL}/citizens`,
    payload,
    { headers: { 'Content-Type': 'application/json' } }
  );
  
  const success = check(response, {
    'register status is 200': (r) => r.status === 200,
    'register returns did': (r) => r.json('did') === did,
  });
  
  errorRate.add(!success);
  
  if (success) {
    return did;
  }
  
  return null;
}

// Digital Identity: Get citizen consents
export function getCitizenConsents() {
  const did = randomDID();
  
  const response = http.get(`${BASE_URL}/citizens/${did}/consents`);
  
  const success = check(response, {
    'consents status is 200 or 404': (r) => r.status === 200 || r.status === 404,
  });
  
  errorRate.add(!success);
}

// Digital Identity: Verify chain
export function verifyChain() {
  const did = randomDID();
  
  const response = http.get(`${BASE_URL}/verify/chain/${did}`);
  
  const success = check(response, {
    'verify chain status is 200 or 404': (r) => r.status === 200 || r.status === 404,
  });
  
  errorRate.add(!success);
}

// Main test scenario
export default function () {
  // Weighted distribution of operations
  const rand = Math.random();
  
  if (rand < 0.1) {
    // 10% - Health checks
    healthCheck();
  } else if (rand < 0.3) {
    // 20% - Challenge creation
    createChallenge();
  } else if (rand < 0.5) {
    // 20% - Response submission
    submitResponse();
  } else if (rand < 0.7) {
    // 20% - List certificates
    listCertificates();
  } else if (rand < 0.8) {
    // 10% - Register citizen
    registerCitizen();
  } else if (rand < 0.9) {
    // 10% - Get citizen consents
    getCitizenConsents();
  } else {
    // 10% - Verify chain
    verifyChain();
  }
  
  sleep(1);
}

// Setup function
export function setup() {
  console.log(`Starting load test against: ${BASE_URL}`);
  
  // Verify server is reachable
  const response = http.get(`${BASE_URL}/health`);
  if (response.status !== 200) {
    throw new Error(`Server not reachable: ${response.status}`);
  }
  
  return { baseUrl: BASE_URL };
}

// Teardown function
export function teardown(data) {
  console.log('Load test complete');
}
