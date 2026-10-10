const crypto = require('crypto');

function createJwt(subject, secret) {
    const header = Buffer.from(JSON.stringify({ alg: 'HS256', typ: 'JWT' })).toString('base64url');
    const payload = Buffer.from(JSON.stringify({ sub: subject, exp: Math.floor(Date.now() / 1000) + 3600 })).toString('base64url');
    const signature = crypto.createHmac('sha256', secret).update(`${header}.${payload}`).digest('base64url');
    return `${header}.${payload}.${signature}`;
}

const JWT_SECRET = 'E9z3i19gKSVQLGOva0bsOpR0Fal3ZmxR';
const BASE_URL = 'http://localhost:9090';
const userId = '11111111-1111-1111-1111-111111111111';
const docId = '22222222-2222-2222-2222-222222222222';
const token = createJwt(userId, JWT_SECRET);

async function runTests() {
    console.log("Using token auth...");
    
    console.log(`\n1. Calling POST /api/v1/kyc/documents/${docId}/extract ...`);
    try {
        let res = await fetch(`${BASE_URL}/api/v1/kyc/documents/${docId}/extract`, {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${token}`
            }
        });
        console.log("Status:", res.status);
        let json = await res.json();
        console.log("Response:", JSON.stringify(json, null, 2));
    } catch(e) { console.error("Extract API Error:", e.message) }

    console.log(`\n2. Calling POST /api/v1/kyc/face-match ...`);
    try {
        let res2 = await fetch(`${BASE_URL}/api/v1/kyc/face-match`, {
            method: 'POST',
            headers: {
                'Authorization': `Bearer ${token}`,
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({
                selfie_url: 's3://bucket/selfie.jpg',
                id_photo_url: 's3://bucket/id_photo.jpg'
            })
        });
        console.log("Status:", res2.status);
        let json2 = await res2.json();
        console.log("Response:", JSON.stringify(json2, null, 2));
    } catch(e) { console.error("Face-match API Error:", e.message) }
}

runTests().catch(console.error);
