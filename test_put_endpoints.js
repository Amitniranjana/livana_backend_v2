const crypto = require('crypto');

function base64url(str) {
    return Buffer.from(str)
        .toString('base64')
        .replace(/=/g, '')
        .replace(/\+/g, '-')
        .replace(/\//g, '_');
}

// Generate token for a valid UUID
const user_id = '9b19975a-b760-47ed-b74b-24afcfd78d85';
const header = { alg: 'HS256', typ: 'JWT' };
const payload = { sub: user_id, exp: Math.floor(Date.now() / 1000) + (60 * 60) };
const encodedHeader = base64url(JSON.stringify(header));
const encodedPayload = base64url(JSON.stringify(payload));
const signatureInput = `${encodedHeader}.${encodedPayload}`;
const signature = crypto.createHmac('sha256', 'E9z3i19gKSVQLGOva0bsOpR0Fal3ZmxR')
    .update(signatureInput)
    .digest('base64')
    .replace(/=/g, '')
    .replace(/\+/g, '-')
    .replace(/\//g, '_');
const token = `${signatureInput}.${signature}`;

const headers = {
    'Content-Type': 'application/json',
    'Authorization': `Bearer ${token}`
};

const BASE_URL = 'http://localhost:9090';

async function fetchAPI(path, method, body) {
    const res = await fetch(`${BASE_URL}${path}`, {
        method,
        headers,
        body: body ? JSON.stringify(body) : undefined
    });
    const text = await res.text();
    try {
        return { status: res.status, data: JSON.parse(text) };
    } catch {
        return { status: res.status, data: text };
    }
}

async function runTests() {
    console.log("🚀 Starting PUT API tests...");
    
    // --- 1. Service Listing ---
    console.log("\n[1] Testing E1: Edit Service Listing");
    const serviceRes = await fetchAPI('/api/services', 'POST', {
        service_name: 'Test Service',
        category: 'cleaning',
        price: 1000,
        description: 'Test description',
        experience: '2 years',
        location: 'Mumbai'
    });
    if (serviceRes.status === 201) {
        const serviceId = serviceRes.data.data.service_id;
        
        // Update single field
        console.log("    - Updating single field (price)");
        const update1 = await fetchAPI(`/api/services/${serviceId}`, 'PUT', { price: 1500 });
        console.log(`      Status: ${update1.status}, Price: ${update1.data.data.price}`);
        
        // Update multiple fields
        console.log("    - Updating multiple fields");
        const update2 = await fetchAPI(`/api/services/${serviceId}`, 'PUT', { service_name: 'Updated Service', experience: '5 years' });
        console.log(`      Status: ${update2.status}, Name: ${update2.data.data.service_name}, Experience: ${update2.data.data.experience}`);
        
        // Invalid data test
        console.log("    - Testing invalid data (price = -500)");
        const update3 = await fetchAPI(`/api/services/${serviceId}`, 'PUT', { price: -500 });
        console.log(`      Status: ${update3.status}, Error: ${update3.data.message}`);
    } else {
        console.error("Failed to create service:", serviceRes.data);
    }
    
    // --- 2. Community ---
    console.log("\n[2] Testing E2: Edit Community");
    const commRes = await fetchAPI('/api/v1/communities', 'POST', {
        name: 'Test Community',
        description: 'Testing community update'
    });
    let commId = null;
    if (commRes.status === 201) {
        commId = commRes.data.data.id;
        
        // Update single field
        console.log("    - Updating single field (description)");
        const update1 = await fetchAPI(`/api/v1/communities/${commId}`, 'PUT', { description: 'Updated desc' });
        console.log(`      Status: ${update1.status}, Description: ${update1.data.data.description}`);
        
        // Test invalid data
        console.log("    - Testing invalid data (empty name)");
        const update2 = await fetchAPI(`/api/v1/communities/${commId}`, 'PUT', { name: '   ' });
        console.log(`      Status: ${update2.status}, Error: ${update2.data.message}`);
    }
    
    // --- 3. Expo Event ---
    console.log("\n[3] Testing E3: Edit Expo Event");
    const expoRes = await fetchAPI('/api/expo', 'POST', {
        title: 'Tech Expo 2026',
        description: 'New Tech',
        location: 'Delhi',
        event_date: '2026-10-10',
        start_time: '09:00',
        end_time: '18:00',
        organizer_id: user_id,
        max_participants: 50
    });
    if (expoRes.status === 201) {
        const expoId = expoRes.data.data.expo_id;
        
        // Update multiple fields
        console.log("    - Updating multiple fields");
        const update1 = await fetchAPI(`/api/expo/${expoId}`, 'PUT', { location: 'Mumbai', max_participants: 100 });
        console.log(`      Status: ${update1.status}, Response: ${update1.data.message}`);
        
        // Invalid data
        console.log("    - Testing invalid data (bad date)");
        const update2 = await fetchAPI(`/api/expo/${expoId}`, 'PUT', { event_date: 'invalid-date' });
        console.log(`      Status: ${update2.status}, Error: ${update2.data.message}`);
    }

    // --- 4. CareCrew Provider ---
    console.log("\n[4] Testing E4: Edit CareCrew Provider");
    // Provider is auto-created when a service is added, but let's just hit the endpoint for user_id
    console.log("    - Updating multiple fields");
    const provRes = await fetchAPI(`/api/v1/carecrew/providers/${user_id}`, 'PUT', {
        bio: 'Expert cleaner',
        phone: '1234567890'
    });
    console.log(`      Status: ${provRes.status}, Bio: ${provRes.data.data?.bio}, Phone: ${provRes.data.data?.phone}`);
    
    console.log("    - Testing invalid data (bad phone)");
    const provRes2 = await fetchAPI(`/api/v1/carecrew/providers/${user_id}`, 'PUT', { phone: '123' });
    console.log(`      Status: ${provRes2.status}, Error: ${provRes2.data.message}`);

    // --- 5. Community Post ---
    console.log("\n[5] Testing E5: Edit Community Post");
    if (commId) {
        const postRes = await fetchAPI(`/api/v1/communities/${commId}/posts`, 'POST', { content: 'Original content' });
        if (postRes.status === 201) {
            const postId = postRes.data.data.post_id;
            
            console.log("    - Updating content");
            const update1 = await fetchAPI(`/api/v1/communities/${commId}/posts/${postId}`, 'PUT', { content: 'Updated content' });
            console.log(`      Status: ${update1.status}, Content: ${update1.data.data.content}`);
            
            console.log("    - Testing invalid data (empty content)");
            const update2 = await fetchAPI(`/api/v1/communities/${commId}/posts/${postId}`, 'PUT', { content: '  ' });
            console.log(`      Status: ${update2.status}, Error: ${update2.data.message}`);
        }
    }

    console.log("\n✅ All Tests Completed!");
    process.exit(0);
}

runTests();
