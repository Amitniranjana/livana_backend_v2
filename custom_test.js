const crypto = require('crypto');

function createJwt(subject, secret) {
    const header = Buffer.from(JSON.stringify({ alg: 'HS256', typ: 'JWT' })).toString('base64url');
    const payload = Buffer.from(JSON.stringify({ sub: subject, exp: Math.floor(Date.now() / 1000) + 3600 })).toString('base64url');
    const signature = crypto.createHmac('sha256', secret).update(`${header}.${payload}`).digest('base64url');
    return `${header}.${payload}.${signature}`;
}

const JWT_SECRET = 'supersecret';
const BASE_URL = 'http://localhost:9090';
// Let's use the ID we inserted: bb9d29e8-a8b7-407a-ba88-9cd8916b9b04
const providerId = 'bb9d29e8-a8b7-407a-ba88-9cd8916b9b04';
const token = createJwt(providerId, JWT_SECRET);

async function runTests() {
    console.log("Using token auth...");
    
    console.log("\n1. Calling POST /api/services ...");
    let res = await fetch(`${BASE_URL}/api/services`, {
        method: 'POST',
        headers: {
            'Authorization': `Bearer ${token}`,
            'Content-Type': 'application/json'
        },
        body: JSON.stringify({
            service_name: 'Dynamic Service Sync Test',
            category: 'electrician',
            price: 1500,
            description: 'This is a test to see if carecrew_providers gets populated.',
            experience: '1 year',
            location: 'Bangalore'
        })
    });
    
    console.log("Status:", res.status);
    let json = await res.json();
    console.log("Response:", JSON.stringify(json, null, 2));

    console.log(`\n2. Calling GET /api/v1/carecrew/providers/${providerId} ...`);
    let res2 = await fetch(`${BASE_URL}/api/v1/carecrew/providers/${providerId}`, {
        method: 'GET',
        headers: {}
    });
    console.log("Status:", res2.status);
    let json2 = await res2.json();
    console.log("Response:", JSON.stringify(json2, null, 2));
}

runTests().catch(console.error);
