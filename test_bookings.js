const axios = require('axios');
const jwt = require('jsonwebtoken');

// Change this based on .env
const BASE_URL = 'http://localhost:8080'; 
const JWT_SECRET = 'supersecret';

// Create a dummy token for testing (you might need a real user ID)
const userId = "00000000-0000-0000-0000-000000000000"; // Assuming postgres accepts all zeros, or just use a real UUID.
const testToken = jwt.sign({ sub: '550e8400-e29b-41d4-a716-446655440000', exp: Math.floor(Date.now() / 1000) + (60 * 60) }, JWT_SECRET);

const headers = { Authorization: `Bearer ${testToken}` };

async function runTests() {
  try {
    console.log('Testing GET /api/bookings ...');
    let res = await axios.get(`${BASE_URL}/api/bookings`, { headers });
    console.log('SUCCESS:', JSON.stringify(res.data, null, 2).substring(0, 500), '...');
  } catch (error) {
    console.error('ERROR /api/bookings:', error.response?.data || error.message);
  }

  try {
    console.log('\nTesting GET /api/bookings/provider ...');
    let res = await axios.get(`${BASE_URL}/api/bookings/provider`, { headers });
    console.log('SUCCESS:', JSON.stringify(res.data, null, 2).substring(0, 500), '...');
  } catch (error) {
    console.error('ERROR /api/bookings/provider:', error.response?.data || error.message);
  }
}

// runTests();
