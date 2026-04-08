const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

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

async function runTest() {
    const userId = "9b19975a-b760-47ed-b74b-24afcfd78d85";
    const jwt = generateJwt(userId);
    let masterChatId = "550e8400-e29b-41d4-a716-446655440000"; // Random fixed test char ID
    
    console.log("=========================================");
    console.log("   TESTING ALL CHAT ENDPOINTS");
    console.log("=========================================\n");

    const headers = { 
        "Authorization": `Bearer ${jwt}`,
        "Content-Type": "application/json"
    };

    // 1. GET /api/chats
    console.log("1. Testing GET /api/chats (List All Chats)...");
    let res = await fetch("http://localhost:9090/api/chats", { headers });
    let data = await res.json();
    console.log("Response:", JSON.stringify(data, null, 2));
    
    // 2. GET /api/v1/chats/recent
    console.log("\n2. Testing GET /api/v1/chats/recent (Recent Chats)...");
    res = await fetch("http://localhost:9090/api/v1/chats/recent", { headers });
    data = await res.json();
    console.log("Response:", JSON.stringify(data, null, 2));

    // 3. POST /chat/messages (Text Message)
    console.log("\n3. Testing POST /chat/messages (Send Text Message via Chime)...");
    const payload = {
        channel_arn: `arn:aws:chime:us-east-1:111111111111:app-instance/123/channel/${masterChatId}`,
        content: "Hello this is a test text message!"
    };
    res = await fetch("http://localhost:9090/chat/messages", {
        method: "POST",
        headers,
        body: JSON.stringify(payload)
    });
    data = await res.json();
    console.log("Response:", JSON.stringify(data, null, 2));

    // 4. POST /api/v1/chats/upload (Media Message)
    console.log("\n4. Testing POST /api/v1/chats/upload (Upload Media)...");
    const uploadFilePath = path.join(__dirname, 'test_doc.txt');
    fs.writeFileSync(uploadFilePath, Buffer.from("dummy doc content", "utf-8"));
    const formData = new FormData();
    const fileBlob = new Blob([fs.readFileSync(uploadFilePath)], { type: 'text/plain' });
    formData.append("file", fileBlob, "test_doc.txt");
    formData.append("chat_id", masterChatId);

    res = await fetch("http://localhost:9090/api/v1/chats/upload", {
        method: "POST",
        headers: { "Authorization": `Bearer ${jwt}` },
        body: formData
    });
    data = await res.json();
    console.log("Response:", JSON.stringify(data, null, 2));
    fs.unlinkSync(uploadFilePath);

    // 5. GET /api/v1/chats/{chat_id}/messages
    console.log(`\n5. Testing GET /api/v1/chats/${masterChatId}/messages (Fetch Chat Messages)...`);
    res = await fetch(`http://localhost:9090/api/v1/chats/${masterChatId}/messages`, { headers });
    data = await res.json();
    console.log("Response:", JSON.stringify(data, null, 2));
    
    console.log("\n✅ ALL ENDPOINTS HAVE BEEN EXECUTED");
    process.exit(0);
}

runTest().catch(console.error);
