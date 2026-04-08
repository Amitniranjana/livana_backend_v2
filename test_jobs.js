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

async function runTest() {
    // 1. JWT Token
    const jwt = generateJwt("9b19975a-b760-47ed-b74b-24afcfd78d85");

    console.log("1. Testing Create Job (POST /api/v1/jobs)...");
    const jobPayload = {
        title: "Senior Flutter Developer",
        description: "Looking for an expert flutter developer",
        location: "Mumbai",
        salary_range: "50-100k USD",
        company_name: "Livana",
        job_type: "Full-Time",
        notice_period: "30 Days"
    };

    const createRes = await fetch("http://localhost:9090/api/v1/jobs", {
        method: "POST",
        headers: { 
            "Authorization": `Bearer ${jwt}`,
            "Content-Type": "application/json"
        },
        body: JSON.stringify(jobPayload)
    });
    const createData = await createRes.json();
    console.log("Create Job Response:", JSON.stringify(createData, null, 2));

    console.log("\n2. Testing List Jobs (GET /api/v1/jobs)...");
    const listRes = await fetch("http://localhost:9090/api/v1/jobs", {
        method: "GET",
    });
    const listData = await listRes.json();
    console.log("List Jobs Response:", JSON.stringify(listData, null, 2));

    // Cleanup and Exit
    process.exit(0);
}

runTest().catch(console.error);
