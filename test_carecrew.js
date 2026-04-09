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

async function testBooking() {
  const token = generateJwt("9b19975a-b760-47ed-b74b-24afcfd78d85");
  console.log("Generated token.");

  // Using the exact payload from the flutter logs
  const payload = {
    provider_id: "149e5792-2f6a-4aba-9124-ed3b8df3f27a",
    service_id: "a343fc81-f35c-494f-a90f-67d40514a418",
    service_type: "plumber",
    scheduled_at: "2026-04-09T08:44:00.000Z"
  };

  console.log("Sending booking request...");
  const bookRes = await fetch('http://localhost:9090/api/v1/carecrew/bookings', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${token}`
    },
    body: JSON.stringify(payload)
  });

  const body = await bookRes.json();
  console.log("Status:", bookRes.status);
  console.log("Response:", JSON.stringify(body, null, 2));
}

testBooking().catch(console.error);
