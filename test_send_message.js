const crypto = require('crypto');

function generateJwt(userId) {
    function base64url(str) {
        return Buffer.from(str).toString('base64').replace(/=/g, '').replace(/\+/g, '-').replace(/\//g, '_');
    }
    const header = { alg: 'HS256', typ: 'JWT' };
    const payload = { sub: userId, exp: Math.floor(Date.now() / 1000) + (60 * 60) };
    const encodedHeader = base64url(JSON.stringify(header));
    const encodedPayload = base64url(JSON.stringify(payload));
    const signatureInput = `${encodedHeader}.${encodedPayload}`;
    const secret = 'E9z3i19gKSVQLGOva0bsOpR0Fal3ZmxR';
    const signature = crypto.createHmac('sha256', secret).update(signatureInput).digest('base64').replace(/=/g, '').replace(/\+/g, '-').replace(/\//g, '_');
    return `${signatureInput}.${signature}`;
}

const BASE = 'http://localhost:9090';
const USER_ID = '9b19975a-b760-47ed-b74b-24afcfd78d85';
const TOKEN = generateJwt(USER_ID);

async function main() {
    console.log('=== Testing POST /chat/messages (Flutter-style payload) ===\n');

    // Exact same payload format as Flutter sends
    const payload = {
        channel_arn: "abbd388c-c1ef-4307-8030-fb29abed62e3",
        content: "Hi"
    };

    console.log('Payload:', JSON.stringify(payload));

    const res = await fetch(`${BASE}/chat/messages`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'Authorization': `Bearer ${TOKEN}`
        },
        body: JSON.stringify(payload)
    });

    const body = await res.json();
    console.log('Status:', res.status);
    console.log('Response:', JSON.stringify(body, null, 2));

    // If message was saved, verify it by fetching messages
    if (res.status === 200) {
        console.log('\n--- Verifying message was saved ---');
        const chatId = payload.channel_arn;  // The UUID itself
        const msgsRes = await fetch(`${BASE}/api/v1/chats/${chatId}/messages`, {
            headers: { 'Authorization': `Bearer ${TOKEN}` }
        });
        const msgs = await msgsRes.json();
        console.log('Messages Status:', msgsRes.status);
        console.log('Messages:', JSON.stringify(msgs, null, 2));
    }
}

main().catch(console.error);
