const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

function generateJwt(userId) {
    function base64url(str) {
        return Buffer.from(str).toString('base64').replace(/=/g, '').replace(/\+/g, '-').replace(/\//g, '_');
    }

    const header = { alg: 'HS256', typ: 'JWT' };
    const payload = {
        sub: userId,
        exp: Math.floor(Date.now() / 1000) + (60 * 60)
    };

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
    const chatId = crypto.randomUUID(); // Mock a new chat ID
    
    // Create dummy image file
    const uploadFilePath = path.join(__dirname, 'test_image.jpg');
    fs.writeFileSync(uploadFilePath, Buffer.from("dummy image content", "utf-8"));

    console.log("1. Testing Upload Media (/api/v1/chats/upload)...");
    
    const formData = new FormData();
    const fileBlob = new Blob([fs.readFileSync(uploadFilePath)], { type: 'image/jpeg' });
    formData.append("file", fileBlob, "test_image.jpg");
    formData.append("chat_id", chatId);

    const uploadRes = await fetch("http://localhost:9090/api/v1/chats/upload", {
        method: "POST",
        headers: { "Authorization": `Bearer ${jwt}` },
        body: formData
    });
    const uploadData = await uploadRes.json();
    console.log("Upload Response:", JSON.stringify(uploadData, null, 2));

    console.log("\n2. Testing Get Messages (/api/v1/chats/" + chatId + "/messages)...");
    const msgsRes = await fetch(`http://localhost:9090/api/v1/chats/${chatId}/messages`, {
        method: "GET",
        headers: { "Authorization": `Bearer ${jwt}` }
    });
    const msgsData = await msgsRes.json();
    console.log("Messages Response:", JSON.stringify(msgsData, null, 2));

    console.log("\n3. Testing Recent Chats (/api/v1/chats/recent)...");
    const recentRes = await fetch("http://localhost:9090/api/v1/chats/recent", {
        method: "GET",
        headers: { "Authorization": `Bearer ${jwt}` }
    });
    const recentData = await recentRes.json();
    console.log("Recent Chats Response:", JSON.stringify(recentData, null, 2));
    
    // Cleanup
    fs.unlinkSync(uploadFilePath);
    process.exit(0);
}

runTest().catch(console.error);
