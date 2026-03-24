const crypto = require('crypto');

function base64url(str) {
    return Buffer.from(str)
        .toString('base64')
        .replace(/=/g, '')
        .replace(/\+/g, '-')
        .replace(/\//g, '_');
}

const header = {
    alg: 'HS256',
    typ: 'JWT'
};

const payload = {
    sub: '00000000-0000-0000-0000-000000000001',
    exp: Math.floor(Date.now() / 1000) + (60 * 60) // 1 hour expiration
};

const encodedHeader = base64url(JSON.stringify(header));
const encodedPayload = base64url(JSON.stringify(payload));
const signatureInput = `${encodedHeader}.${encodedPayload}`;

const secret = 'E9z3i19gKSVQLGOva0bsOpR0Fal3ZmxR'; // from .env JWT_SECRET_KEY but it might fallback to supersecret. 
// Wait, the Rust code uses app_state.jwt_secret which is loaded using env::var("JWT_SECRET").unwrap_or("supersecret")
const signature = crypto.createHmac('sha256', 'supersecret')
    .update(signatureInput)
    .digest('base64')
    .replace(/=/g, '')
    .replace(/\+/g, '-')
    .replace(/\//g, '_');

const jwt = `${signatureInput}.${signature}`;
console.log(jwt);
