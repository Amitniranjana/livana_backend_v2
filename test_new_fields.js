const crypto = require('crypto');

function createJwt(subject, secret) {
    const header = Buffer.from(JSON.stringify({ alg: 'HS256', typ: 'JWT' })).toString('base64url');
    const payload = Buffer.from(JSON.stringify({ sub: subject, exp: Math.floor(Date.now() / 1000) + 3600 })).toString('base64url');
    const signature = crypto.createHmac('sha256', secret).update(`${header}.${payload}`).digest('base64url');
    return `${header}.${payload}.${signature}`;
}

const JWT_SECRET = 'E9z3i19gKSVQLGOva0bsOpR0Fal3ZmxR';
const BASE_URL = 'http://localhost:9090';

// We just need a dummy UUID for the user since the DB was dropped, we need to create the user first
const u_id = '11111111-1111-1111-1111-111111111111';
const token = createJwt(u_id, JWT_SECRET);

async function request(method, path, token, body = null) {
    const res = await fetch(`${BASE_URL}${path}`, {
        method,
        headers: {
            'Authorization': `Bearer ${token}`,
            'Content-Type': 'application/json'
        },
        body: body ? JSON.stringify(body) : undefined
    });
    const text = await res.text();
    let json;
    try {
        json = JSON.parse(text);
    } catch (e) {
        json = text; 
    }
    return { status: res.status, json };
}

async function run() {
    // Re-create the user locally since database drop wiped it
    const { execSync } = require('child_process');
    execSync(`psql -h localhost -p 5433 -U postgres -d livana_db -c "INSERT INTO users (id, first_name, last_name, email, phone_no, password) VALUES ('${u_id}', 'Test', 'User', 'test@example.com', '1234567890', 'hash') ON CONFLICT DO NOTHING;"`, {
        env: { ...process.env, PAGER: "", PGPASSWORD: "password1235" }
    });

    console.log("=== Creating Property ===");
    const res = await request('POST', '/api/properties', token, {
        title: "Test Extended Property",
        description: "Testing new fields",
        property_type: "Rent",
        price: 12345556,
        deposit: 123455,
        location: "Union Square",
        area_sqft: 1234,
        bedrooms: 1,
        bathrooms: 1,
        floor: 1,
        total_floors: 1,
        age_years: 0,
        facing: "West",
        parking: false,
        broker_contact_allowed: true,
        user_type: "user"
    });
    console.log("Create POST Status:", res.status);
    console.log("Create Response:", JSON.stringify(res.json, null, 2));

    if (res.status === 201) {
        const p_id = res.json.data.property_id;
        console.log(`\n=== Fetching Property ${p_id} ===`);
        const getRes = await request('GET', `/api/properties/${p_id}`, token);
        console.log("GET Status:", getRes.status);
        console.log("Property Deposit:", getRes.json.data.property.deposit);
        console.log("Property Floor:", getRes.json.data.property.floor);
        console.log("Property Age:", getRes.json.data.property.age_years);
        console.log("Property user_type:", getRes.json.data.property.user_type);
        console.log("Property broker_contact_allowed:", getRes.json.data.property.broker_contact_allowed);

        console.log("\n=== Updating Property " + p_id + " ===");
        const putRes = await request('PUT', '/api/properties/' + p_id, token, {
            deposit: 500,
            floor: 2,
            age_years: 5,
            user_type: "broker",
            broker_contact_allowed: false
        });
        console.log("PUT Status:", putRes.status);
        
        console.log("\n=== Fetching Updated Property ===");
        const getUpdatedRes = await request('GET', '/api/properties/' + p_id, token);
        console.log("Updated Deposit:", getUpdatedRes.json.data.property.deposit);
        console.log("Updated Floor:", getUpdatedRes.json.data.property.floor);
        console.log("Updated Age:", getUpdatedRes.json.data.property.age_years);
        console.log("Updated user_type:", getUpdatedRes.json.data.property.user_type);
        console.log("Updated broker_contact_allowed:", getUpdatedRes.json.data.property.broker_contact_allowed);
    }
}
run();
