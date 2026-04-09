const crypto = require('crypto');

// ── Config ──
const BASE = "http://localhost:9090";
const JWT_SECRET = "E9z3i19gKSVQLGOva0bsOpR0Fal3ZmxR";
const USER_ID = "9b19975a-b760-47ed-b74b-24afcfd78d85";

function generateJwt(userId) {
    function b64url(str) {
        return Buffer.from(str).toString('base64').replace(/=/g, '').replace(/\+/g, '-').replace(/\//g, '_');
    }
    const header = { alg: 'HS256', typ: 'JWT' };
    const payload = { sub: userId, exp: Math.floor(Date.now() / 1000) + 3600 };
    const input = `${b64url(JSON.stringify(header))}.${b64url(JSON.stringify(payload))}`;
    const sig = crypto.createHmac('sha256', JWT_SECRET).update(input).digest('base64')
        .replace(/=/g, '').replace(/\+/g, '-').replace(/\//g, '_');
    return `${input}.${sig}`;
}

const JWT = generateJwt(USER_ID);
const AUTH = { "Authorization": `Bearer ${JWT}`, "Content-Type": "application/json" };

async function test() {
    console.log("═══════════════════════════════════════════════");
    console.log("  PROPERTY LIKES (is_liked) FIX — INTEGRATION TEST");
    console.log("═══════════════════════════════════════════════\n");

    // 1. List properties to pick a property_id
    console.log("1️⃣  GET /api/properties — Fetching property list...");
    let res = await fetch(`${BASE}/api/properties`, { headers: AUTH });
    let data = await res.json();
    if (!data.success || !data.data?.properties?.length) {
        console.log("❌ No properties found. Cannot test likes.");
        console.log("   Response:", JSON.stringify(data, null, 2));
        process.exit(1);
    }
    const prop = data.data.properties[0];
    const PROP_ID = prop.id;
    console.log(`   ✅ Found ${data.data.properties.length} properties. Using: ${PROP_ID}`);
    console.log(`   Current is_liked: ${prop.is_liked}, likes_count: ${prop.likes_count}\n`);

    // 2. Unlike first (cleanup from previous runs)
    console.log("2️⃣  DELETE /api/properties/{id}/like — Cleaning up previous like...");
    res = await fetch(`${BASE}/api/properties/${PROP_ID}/like`, { method: "DELETE", headers: AUTH });
    data = await res.json();
    console.log(`   Status: ${res.status} — ${data.message}\n`);

    // 3. Verify is_liked = false after unlike
    console.log("3️⃣  GET /api/properties/{id} — Verify is_liked = false after unlike...");
    res = await fetch(`${BASE}/api/properties/${PROP_ID}`, { headers: AUTH });
    data = await res.json();
    const beforeLike = data.data?.property?.is_liked;
    const beforeCount = data.data?.property?.likes_count;
    console.log(`   is_liked: ${beforeLike}, likes_count: ${beforeCount}`);
    if (beforeLike !== false) {
        console.log("   ⚠️  Expected is_liked=false after unlike");
    } else {
        console.log("   ✅ Correctly false\n");
    }

    // 4. Like the property
    console.log("4️⃣  POST /api/properties/{id}/like — Liking the property...");
    res = await fetch(`${BASE}/api/properties/${PROP_ID}/like`, { method: "POST", headers: AUTH });
    data = await res.json();
    console.log(`   Status: ${res.status} — ${data.message}`);
    console.log(`   liked: ${data.data?.liked}, likes_count: ${data.data?.likes_count}\n`);

    // 5. Verify is_liked = true now on GET single
    console.log("5️⃣  GET /api/properties/{id} — Verify is_liked = true after like...");
    res = await fetch(`${BASE}/api/properties/${PROP_ID}`, { headers: AUTH });
    data = await res.json();
    const afterLike = data.data?.property?.is_liked;
    const afterCount = data.data?.property?.likes_count;
    console.log(`   is_liked: ${afterLike}, likes_count: ${afterCount}`);
    if (afterLike === true) {
        console.log("   ✅ PASS: is_liked is correctly true!\n");
    } else {
        console.log("   ❌ FAIL: is_liked is still false!\n");
    }

    // 6. Verify is_liked = true on the list endpoint too
    console.log("6️⃣  GET /api/properties — Verify is_liked in list endpoint...");
    res = await fetch(`${BASE}/api/properties`, { headers: AUTH });
    data = await res.json();
    const listedProp = data.data?.properties?.find(p => p.id === PROP_ID);
    if (listedProp) {
        console.log(`   is_liked: ${listedProp.is_liked}, likes_count: ${listedProp.likes_count}`);
        if (listedProp.is_liked === true) {
            console.log("   ✅ PASS: is_liked is correctly true in list!\n");
        } else {
            console.log("   ❌ FAIL: is_liked is false in list!\n");
        }
    } else {
        console.log("   ⚠️  Property not found in list\n");
    }

    // 7. Unlike and verify cleanup
    console.log("7️⃣  DELETE /api/properties/{id}/like — Unlike (cleanup)...");
    res = await fetch(`${BASE}/api/properties/${PROP_ID}/like`, { method: "DELETE", headers: AUTH });
    data = await res.json();
    console.log(`   Status: ${res.status} — ${data.message}`);

    res = await fetch(`${BASE}/api/properties/${PROP_ID}`, { headers: AUTH });
    data = await res.json();
    console.log(`   is_liked after unlike: ${data.data?.property?.is_liked}`);
    if (data.data?.property?.is_liked === false) {
        console.log("   ✅ PASS: unlike working correctly!\n");
    } else {
        console.log("   ❌ FAIL: unlike did not reset is_liked\n");
    }

    console.log("═══════════════════════════════════════════════");
    console.log("  TEST COMPLETE");
    console.log("═══════════════════════════════════════════════");
    process.exit(0);
}

test().catch(e => { console.error("Test Error:", e.message); process.exit(1); });
