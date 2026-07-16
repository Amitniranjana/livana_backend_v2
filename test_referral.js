const fetch = require('node-fetch'); // wait node 18+ has fetch built-in
async function run() {
    const baseUrl = 'http://localhost:9090';
    
    const userA = {
        firstName: 'UserA',
        lastName: 'Test',
        email: `usera_${Date.now()}@test.com`,
        password: 'password123',
        phoneNo: `+9199${Math.floor(10000000 + Math.random() * 90000000)}`,
        gender: 'male',
        userRole: 'user'
    };

    console.log('Signing up User A:', userA.email);
    let resA = await fetch(`${baseUrl}/api/auth/signup`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(userA)
    });
    let dataA = await resA.json();
    console.log('User A Signup Response:', dataA);
    if (!dataA.success) return console.error('Failed to sign up A');
    
    let tokenA = dataA.data.token;

    console.log('\nGetting Referral Info for User A');
    let refRes = await fetch(`${baseUrl}/api/v1/referrals/me`, {
        headers: { 'Authorization': `Bearer ${tokenA}` }
    });
    let refData = await refRes.json();
    console.log('Referral Info:', refData);
    let refCode = refData.data.referralCode;

    const userB = {
        firstName: 'UserB',
        lastName: 'Test',
        email: `userb_${Date.now()}@test.com`,
        password: 'password123',
        phoneNo: `+9199${Math.floor(10000000 + Math.random() * 90000000)}`,
        gender: 'male',
        userRole: 'user',
        refCode: refCode
    };

    console.log(`\nSigning up User B using refCode: ${refCode}`);
    let resB = await fetch(`${baseUrl}/api/auth/signup`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(userB)
    });
    let dataB = await resB.json();
    console.log('User B Signup Response:', dataB);
    if (!dataB.success) return console.error('Failed to sign up B');

    console.log('\nGetting Referral Info for User A again to see if pending increased');
    let refRes2 = await fetch(`${baseUrl}/api/v1/referrals/me`, {
        headers: { 'Authorization': `Bearer ${tokenA}` }
    });
    let refData2 = await refRes2.json();
    console.log('Referral Info After User B signup:', refData2);
}
run();
