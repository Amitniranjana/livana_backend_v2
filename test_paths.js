const token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiI5YjE5OTc1YS1iNzYwLTQ3ZWQtYjc0Yi0yNGFmY2ZkNzhkODUiLCJleHAiOjE3NzU3NjMzOTl9.dFWxxVY-a3J-bk6SZ__1oIrqh3jzVxrN0vfYd0y9TEc";
const localChatUrl = "http://localhost:8000/api/v1/chats/local_chat_7768a0b8-df2f-4273-b2fe-dc48038cbe19_149e5792-2f6a-4aba-9124-ed3b8df3f27a_electrician/messages";
const arnChatUrl = "http://localhost:8000/api/v1/chats/arn:aws:chime:us-east-1:444215322073:app-instance/1f5565f3-c3ad-427a-9cc2-82665109823d/channel/593bbf7e-392d-405e-8b6b-7ef3e0aed39d/messages";

async function runTests() {
    console.log("Testing Local Chat Path...");
    try {
        const res = await fetch(localChatUrl, { headers: { Authorization: `Bearer ${token}` } });
        const text = await res.text();
        console.log(`Status: ${res.status}`);
        console.log(`Response: ${text}\n`);
    } catch (e) {
        console.error("Local chat error:", e);
    }

    console.log("Testing ARN Chat Path...");
    try {
        const res2 = await fetch(arnChatUrl, { headers: { Authorization: `Bearer ${token}` } });
        const text2 = await res2.text();
        console.log(`Status: ${res2.status}`);
        console.log(`Response: ${text2}\n`);
    } catch (e) {
        console.error("ARN chat error:", e);
    }
}

runTests();
