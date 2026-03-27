const { execSync } = require('child_process');
const crypto = require('crypto');

// Generate a valid JWT manually using HS256
function createJwt(subject, secret) {
    const header = Buffer.from(JSON.stringify({ alg: 'HS256', typ: 'JWT' })).toString('base64url');
    const payload = Buffer.from(JSON.stringify({ sub: subject, exp: Math.floor(Date.now() / 1000) + 3600 })).toString('base64url');
    const signature = crypto.createHmac('sha256', secret).update(`${header}.${payload}`).digest('base64url');
    return `${header}.${payload}.${signature}`;
}

const JWT_SECRET = 'supersecret';
const BASE_URL = 'http://localhost:9090';

const u_provider = '11111111-1111-1111-1111-111111111111';
const u_customer = '22222222-2222-2222-2222-222222222222';
const p_id       = '33333333-3333-3333-3333-333333333333';
const b_id       = '44444444-4444-4444-4444-444444444444';
const v_id       = '55555555-5555-5555-5555-555555555555';
const s_id       = '66666666-6666-6666-6666-666666666666';

const providerToken = createJwt(u_provider, JWT_SECRET);
const customerToken = createJwt(u_customer, JWT_SECRET);

console.log("=== Seeding Data via PSQL ===");
const seedSql = `
DO $$
BEGIN
    DELETE FROM users WHERE id IN ('${u_provider}', '${u_customer}');
    
    INSERT INTO users (id, first_name, last_name, email, phone_no, password, user_role, verified, status, created_at, updated_at)
    VALUES 
    ('${u_provider}', 'Test', 'Provider', 'testprov@example.com', '1000000001', 'hash', 'carecrew', true, 'active', NOW(), NOW()),
    ('${u_customer}', 'Test', 'Customer', 'testcust@example.com', '1000000002', 'hash', 'customer', true, 'active', NOW(), NOW());

    INSERT INTO carecrew_services (id, name, is_active) 
    VALUES ('${s_id}', 'Test Service', TRUE) ON CONFLICT DO NOTHING;

    INSERT INTO carecrew_providers (id, name, bio, service_type, city, user_id, created_at)
    VALUES ('${u_provider}', 'Test Provider', 'Bio', 'plumbing', 'Mumbai', '${u_provider}', NOW())
    ON CONFLICT DO NOTHING;

    INSERT INTO properties (id, user_id, title, city, property_type, price, created_at)
    VALUES ('${p_id}', '${u_provider}', 'Test Property', 'Test City', 'flat', 10000, NOW())
    ON CONFLICT DO NOTHING;

    INSERT INTO carecrew_bookings (id, provider_id, service_id, user_id, scheduled_at, status, created_at)
    VALUES ('${b_id}', '${u_provider}', '${s_id}', '${u_customer}', NOW(), 'completed', NOW())
    ON CONFLICT DO NOTHING;

    INSERT INTO site_visits (id, property_id, provider_id, user_id, scheduled_date_time, status, created_at, updated_at)
    VALUES ('${v_id}', '${p_id}', '${u_provider}', '${u_customer}', NOW(), 'completed', NOW(), NOW())
    ON CONFLICT DO NOTHING;
END $$;
`;

try {
    execSync(`psql -h localhost -p 5433 -U postgres -d livana_db -c "${seedSql.replace(/\n/g, ' ')}"`, {
        env: { ...process.env, PAGER: "", PGPASSWORD: "password1235" }
    });
    console.log("✅ Seed data inserted successfully");
} catch (e) {
    console.error("❌ Failed to seed data:", e.message);
    process.exit(1);
}

let cReviewId = null;
let pReviewId = null;

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
        json = text; // Just return the raw text if not JSON
    }
    return { status: res.status, json };
}

async function runTests() {
    const results = [];
    console.log("\n=== Testing APIs ===");

    // 1. ADD SERVICE
    const res1 = await request('POST', '/api/services', providerToken, {
        service_name: 'Emergency Pipe Fixing',
        category: 'plumber',
        price: 500,
        description: 'Fixing pipes fast',
        experience: '5 years',
        location: 'Mumbai'
    });
    results.push({ endpoint: "POST /api/services", status: res1.status, res: res1 });

    // 2. GET ALL SERVICES
    const res2 = await request('GET', '/api/services', customerToken);
    results.push({ endpoint: "GET /api/services", status: res2.status, res: res2 });

    // 3. FILTER PROVIDERS
    const res3 = await request('GET', '/api/services/providers?service_type=plumber&sort_by=price', customerToken);
    results.push({ endpoint: "GET /api/services/providers", status: res3.status, res: res3 });

    // 4. POST CARECREW REVIEW
    const res4 = await request('POST', '/api/reviews/carecrew', customerToken, {
        booking_id: b_id,
        provider_id: u_provider,
        rating: 4.5,
        comment: 'Great work!'
    });
    if (res4.status === 201) {
        cReviewId = res4.json.data.review_id;
    }
    results.push({ endpoint: "POST /api/reviews/carecrew", status: res4.status, res: res4 });

    // 5. GET CARECREW REVIEWS
    const res5 = await request('GET', `/api/reviews/carecrew/${u_provider}`, providerToken);
    results.push({ endpoint: "GET /api/reviews/carecrew/:id", status: res5.status, res: res5 });

    // 6. PUT CARECREW REVIEW
    const res6 = await request('PUT', `/api/reviews/carecrew/${cReviewId}`, customerToken, { rating: 5.0 });
    results.push({ endpoint: "PUT /api/reviews/carecrew/:id", status: res6.status, res: res6 });

    // 7. POST CARECREW REPLY
    const res7 = await request('POST', `/api/reviews/carecrew/${cReviewId}/reply`, providerToken, { reply: "Thank you!" });
    results.push({ endpoint: "POST /api/reviews/carecrew/:id/reply", status: res7.status, res: res7 });

    // 8. DELETE CARECREW REVIEW
    const res8 = await request('DELETE', `/api/reviews/carecrew/${cReviewId}`, customerToken);
    results.push({ endpoint: "DELETE /api/reviews/carecrew/:id", status: res8.status, res: res8 });

    // 9. POST PROPERTY REVIEW
    const res9 = await request('POST', '/api/reviews/property', customerToken, {
        visit_id: v_id,
        property_id: p_id,
        rating: 4.0,
        comment: 'Nice place',
        location_rating: 4.5
    });
    if (res9.status === 201) {
        pReviewId = res9.json.data.review_id;
    }
    results.push({ endpoint: "POST /api/reviews/property", status: res9.status, res: res9 });

    // 10. GET PROPERTY REVIEWS
    const res10 = await request('GET', `/api/reviews/property/${p_id}`, customerToken);
    results.push({ endpoint: "GET /api/reviews/property/:id", status: res10.status, res: res10 });

    // 11. PUT PROPERTY REVIEW
    const res11 = await request('PUT', `/api/reviews/property/${pReviewId}`, customerToken, { cleanliness_rating: 4.8 });
    results.push({ endpoint: "PUT /api/reviews/property/:id", status: res11.status, res: res11 });

    // 12. POST PROPERTY REPLY
    const res12 = await request('POST', `/api/reviews/property/${pReviewId}/reply`, providerToken, { reply: "Glad you liked it!" });
    results.push({ endpoint: "POST /api/reviews/property/:id/reply", status: res12.status, res: res12 });

    // 13. DELETE PROPERTY REVIEW
    const res13 = await request('DELETE', `/api/reviews/property/${pReviewId}`, customerToken);
    results.push({ endpoint: "DELETE /api/reviews/property/:id", status: res13.status, res: res13 });

    require('fs').writeFileSync('test_results.json', JSON.stringify(results, null, 2));
    console.log("✅ Results written to test_results.json");
}

runTests();
