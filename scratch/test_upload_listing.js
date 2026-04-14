const crypto = require('crypto');
const fs = require('fs');
const http = require('http');

function base64url(str) {
    return Buffer.from(str)
        .toString('base64')
        .replace(/=/g, '')
        .replace(/\+/g, '-')
        .replace(/\//g, '_');
}

// 1. Generate JWT
const secret = 'E9z3i19gKSVQLGOva0bsOpR0Fal3ZmxR';
const header = { alg: 'HS256', typ: 'JWT' };
const payload = {
    sub: '9b19975a-b760-47ed-b74b-24afcfd78d85',
    exp: Math.floor(Date.now() / 1000) + (60 * 60)
};

const encodedHeader = base64url(JSON.stringify(header));
const encodedPayload = base64url(JSON.stringify(payload));
const signatureInput = `${encodedHeader}.${encodedPayload}`;
const signature = crypto.createHmac('sha256', secret)
    .update(signatureInput)
    .digest('base64')
    .replace(/=/g, '')
    .replace(/\+/g, '-')
    .replace(/\//g, '_');

const token = `${signatureInput}.${signature}`;
console.log('Generated Token:', token);

// 2. Create dummy image
const dummyPath = 'test_image.jpg';
fs.writeFileSync(dummyPath, Buffer.alloc(1024, 'a')); // 1KB dummy file

// 3. Prepare Multipart Request
const boundary = '----TestBoundary' + Math.random().toString(36).substring(2);
const CRLF = '\r\n';

let body = Buffer.alloc(0);

// Add listing_type
body = Buffer.concat([
    body,
    Buffer.from(`--${boundary}${CRLF}`),
    Buffer.from(`Content-Disposition: form-data; name="listing_type"${CRLF}${CRLF}`),
    Buffer.from(`property${CRLF}`)
]);

// Add files
const fileHeader = `--${boundary}${CRLF}Content-Disposition: form-data; name="files"; filename="test_image.jpg"${CRLF}Content-Type: image/jpeg${CRLF}${CRLF}`;
body = Buffer.concat([
    body,
    Buffer.from(fileHeader),
    fs.readFileSync(dummyPath),
    Buffer.from(CRLF)
]);

body = Buffer.concat([
    body,
    Buffer.from(`--${boundary}--${CRLF}`)
]);

const options = {
    hostname: '127.0.0.1',
    port: 9090,
    path: '/api/listings/upload/images',
    method: 'POST',
    headers: {
        'Authorization': `Bearer ${token}`,
        'Content-Type': `multipart/form-data; boundary=${boundary}`,
        'Content-Length': body.length
    }
};

const req = http.request(options, (res) => {
    let responseData = '';
    res.on('data', (chunk) => { responseData += chunk; });
    res.on('end', () => {
        console.log('Status Code:', res.statusCode);
        console.log('Response Body:', responseData);
        fs.unlinkSync(dummyPath);
    });
});

req.on('error', (e) => {
    console.error('Request Error (Check if server is running):', e.message);
});

req.write(body);
req.end();
