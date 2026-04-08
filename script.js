const jwt = require('jsonwebtoken');
const token = jwt.sign({ sub: '74039329-e27d-418e-aee3-3e322409289f' }, 'livana_backend_secret_key_123', { expiresIn: '100y' });
console.log(token);
