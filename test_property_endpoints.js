const crypto = require('crypto');

// ─── Config ───────────────────────────────────────────────────────────────────
const BASE_URL    = 'http://localhost:9090';
const JWT_SECRET  = 'supersecret';
const USER_ID     = '9b19975a-b760-47ed-b74b-24afcfd78d85'; // Amit (from DB)
const PROPERTY_ID = '2e911e22-50b8-435e-b46b-0120f2cfb127'; // active listing

// ─── JWT helper ───────────────────────────────────────────────────────────────
function makeJwt(subject) {
  const h = Buffer.from(JSON.stringify({ alg: 'HS256', typ: 'JWT' })).toString('base64url');
  const p = Buffer.from(JSON.stringify({ sub: subject, exp: Math.floor(Date.now()/1000)+3600 })).toString('base64url');
  const sig = crypto.createHmac('sha256', JWT_SECRET).update(`${h}.${p}`).digest('base64url');
  return `${h}.${p}.${sig}`;
}

const TOKEN = makeJwt(USER_ID);
const AUTH  = { 'Authorization': `Bearer ${TOKEN}`, 'Content-Type': 'application/json' };
const ANON  = { 'Content-Type': 'application/json' };

// ─── Helpers ──────────────────────────────────────────────────────────────────
let passed = 0, failed = 0;

async function test(label, fn) {
  try {
    await fn();
    console.log(`  ✅  ${label}`);
    passed++;
  } catch(e) {
    console.log(`  ❌  ${label}`);
    console.log(`       ${e.message}`);
    failed++;
  }
}

function assert(cond, msg) { if (!cond) throw new Error(msg); }
function assertField(obj, field, expected) {
  assert(obj[field] !== undefined, `missing field "${field}"`);
  if (expected !== undefined) assert(obj[field] === expected, `"${field}" expected ${JSON.stringify(expected)}, got ${JSON.stringify(obj[field])}`);
}

async function req(method, path, body, headers = AUTH) {
  const r = await fetch(`${BASE_URL}${path}`, {
    method, headers,
    body: body ? JSON.stringify(body) : undefined
  });
  const json = await r.json().catch(() => ({}));
  return { status: r.status, body: json };
}

// ─── Test Suites ──────────────────────────────────────────────────────────────
async function main() {
  console.log('\n═══════════════════════════════════════════════════');
  console.log('  Property Endpoint Tests');
  console.log('═══════════════════════════════════════════════════\n');

  // ── 1. GET /api/properties (anonymous) ─────────────────────────────────────
  console.log('  1. GET /api/properties  — anonymous (is_saved + is_liked must be false)');
  await test('Response is 200', async () => {
    const { status } = await req('GET', '/api/properties', null, ANON);
    assert(status === 200, `Expected 200, got ${status}`);
  });
  await test('Properties array exists', async () => {
    const { body } = await req('GET', '/api/properties', null, ANON);
    assert(Array.isArray(body.data?.properties), 'data.properties is not an array');
  });
  await test('is_saved = false for anonymous', async () => {
    const { body } = await req('GET', '/api/properties', null, ANON);
    const props = body.data?.properties ?? [];
    if (props.length > 0) assertField(props[0], 'is_saved', false);
    else console.log('       (no properties to check, skipping)');
  });
  await test('is_liked = false for anonymous', async () => {
    const { body } = await req('GET', '/api/properties', null, ANON);
    const props = body.data?.properties ?? [];
    if (props.length > 0) assertField(props[0], 'is_liked', false);
    else console.log('       (no properties to check, skipping)');
  });
  await test('no_of_toilets field present', async () => {
    const { body } = await req('GET', '/api/properties', null, ANON);
    const props = body.data?.properties ?? [];
    if (props.length > 0) assertField(props[0], 'no_of_toilets');
    else console.log('       (no properties to check, skipping)');
  });
  await test('no_of_balconies field present', async () => {
    const { body } = await req('GET', '/api/properties', null, ANON);
    const props = body.data?.properties ?? [];
    if (props.length > 0) assertField(props[0], 'no_of_balconies');
    else console.log('       (no properties to check, skipping)');
  });

  // ── 2. GET /api/properties/{id} ────────────────────────────────────────────
  console.log(`\n  2. GET /api/properties/${PROPERTY_ID}  — single property`);
  await test('Returns 200', async () => {
    const { status } = await req('GET', `/api/properties/${PROPERTY_ID}`, null, AUTH);
    assert(status === 200, `Expected 200, got ${status}`);
  });
  await test('has is_saved field', async () => {
    const { body } = await req('GET', `/api/properties/${PROPERTY_ID}`, null, AUTH);
    assertField(body.data?.property, 'is_saved');
  });
  await test('has is_liked field', async () => {
    const { body } = await req('GET', `/api/properties/${PROPERTY_ID}`, null, AUTH);
    assertField(body.data?.property, 'is_liked');
  });
  await test('has no_of_toilets field', async () => {
    const { body } = await req('GET', `/api/properties/${PROPERTY_ID}`, null, AUTH);
    assertField(body.data?.property, 'no_of_toilets');
  });
  await test('has no_of_balconies field', async () => {
    const { body } = await req('GET', `/api/properties/${PROPERTY_ID}`, null, AUTH);
    assertField(body.data?.property, 'no_of_balconies');
  });

  // ── 3. POST /api/properties  — create with new fields ──────────────────────
  console.log('\n  3. POST /api/properties  — create with toilets & balconies');
  let createdId = null;
  await test('Creates property (201)', async () => {
    const { status, body } = await req('POST', '/api/properties', {
      title: '3BHK Luxury Apt with Balcony Test',
      description: 'Integration test property',
      property_type: 'rent',
      price: 35000,
      location: 'Bandra West, Mumbai',
      bedrooms: 3,
      bathrooms: 3,
      no_of_toilets: 2,
      no_of_balconies: 1,
      parking: true,
      user_type: 'user'
    });
    assert(status === 201, `Expected 201, got ${status} — ${JSON.stringify(body)}`);
    createdId = body.data?.property_id;
    assert(createdId, 'No property_id in response');
  });
  await test('Created property has correct toilets & balconies', async () => {
    if (!createdId) throw new Error('No property created in previous step');
    const { body } = await req('GET', `/api/properties/${createdId}`, null, AUTH);
    assertField(body.data?.property, 'no_of_toilets', 2);
    assertField(body.data?.property, 'no_of_balconies', 1);
  });

  // ── 4. POST /api/properties/{id}/like ──────────────────────────────────────
  console.log(`\n  4. POST /api/properties/${PROPERTY_ID}/like  — Like a property`);
  await test('Like returns 200', async () => {
    const { status, body } = await req('POST', `/api/properties/${PROPERTY_ID}/like`, null, AUTH);
    assert(status === 200, `Expected 200, got ${status} — ${JSON.stringify(body)}`);
  });
  await test('is_liked = true after liking', async () => {
    const { body } = await req('GET', `/api/properties/${PROPERTY_ID}`, null, AUTH);
    assertField(body.data?.property, 'is_liked', true);
  });
  await test('is_liked = false for anonymous after like', async () => {
    const { body } = await req('GET', `/api/properties/${PROPERTY_ID}`, null, ANON);
    assertField(body.data?.property, 'is_liked', false);
  });

  // ── 5. DELETE /api/properties/{id}/like ────────────────────────────────────
  console.log(`\n  5. DELETE /api/properties/${PROPERTY_ID}/like  — Unlike`);
  await test('Unlike returns 200', async () => {
    const { status, body } = await req('DELETE', `/api/properties/${PROPERTY_ID}/like`, null, AUTH);
    assert(status === 200, `Expected 200, got ${status} — ${JSON.stringify(body)}`);
  });
  await test('is_liked = false after unliking', async () => {
    const { body } = await req('GET', `/api/properties/${PROPERTY_ID}`, null, AUTH);
    assertField(body.data?.property, 'is_liked', false);
  });

  // ── 6. POST /api/properties/{id}/save ──────────────────────────────────────
  console.log(`\n  6. POST /api/properties/${PROPERTY_ID}/save  — Save a property`);
  await test('Save returns 200', async () => {
    const { status, body } = await req('POST', `/api/properties/${PROPERTY_ID}/save`, null, AUTH);
    assert(status === 200, `Expected 200, got ${status} — ${JSON.stringify(body)}`);
  });
  await test('is_saved = true after saving', async () => {
    const { body } = await req('GET', `/api/properties/${PROPERTY_ID}`, null, AUTH);
    assertField(body.data?.property, 'is_saved', true);
  });
  await test('is_saved = false for anonymous after save', async () => {
    const { body } = await req('GET', `/api/properties/${PROPERTY_ID}`, null, ANON);
    assertField(body.data?.property, 'is_saved', false);
  });

  // ── 7. DELETE /api/properties/{id}/save ────────────────────────────────────
  console.log(`\n  7. DELETE /api/properties/${PROPERTY_ID}/save  — Unsave`);
  await test('Unsave returns 200', async () => {
    const { status, body } = await req('DELETE', `/api/properties/${PROPERTY_ID}/save`, null, AUTH);
    assert(status === 200, `Expected 200, got ${status} — ${JSON.stringify(body)}`);
  });
  await test('is_saved = false after unsaving', async () => {
    const { body } = await req('GET', `/api/properties/${PROPERTY_ID}`, null, AUTH);
    assertField(body.data?.property, 'is_saved', false);
  });

  // ── 8. Cleanup: delete test property ───────────────────────────────────────
  if (createdId) {
    await req('DELETE', `/api/properties/${createdId}`, null, AUTH);
  }

  // ── Results ────────────────────────────────────────────────────────────────
  console.log('\n═══════════════════════════════════════════════════');
  console.log(`  Results: ${passed} passed  |  ${failed} failed`);
  console.log('═══════════════════════════════════════════════════\n');
  if (failed > 0) process.exit(1);
}

main().catch(e => { console.error(e); process.exit(1); });
