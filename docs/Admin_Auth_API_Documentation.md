# Admin Authentication API — Frontend Documentation

> **Module:** Admin Auth  
> **Base Path:** `/api/admin`  
> **Last Updated:** 2026-08-01

---

## Overview

Yeh module admin panel ka authentication handle karta hai. Teen endpoints hain:

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/admin/login` | Admin login karo, session cookie set hoti hai |
| `POST` | `/api/admin/logout` | Session cookie clear hoti hai |
| `GET` | `/api/admin/me` | Current logged-in admin ki info lo |

Authentication **dual-mode** hai — server **cookie-based session** aur **Bearer token** dono accept karta hai. Cookie wala approach recommended hai browser apps ke liye.

---

## ⚠️ Local Dev vs Production — Cookie Behavior

> **Yeh section padna zaroori hai — warna login "succeed" karega lekin 401 aate rahenge.**

### Problem

Backend production mein `Secure` + `SameSite=None` cookies set karta tha. Yeh **HTTPS pe hi kaam karta hai**. Agar aap local dev pe plain `http://localhost` use kar rahe ho, browser cookie silently reject karta tha, aur har request pe `401 Unauthorized` aata tha even after successful login.

### Fix (Jo Humne Kiya)

Backend mein `COOKIE_SECURE` env variable support add kiya gaya hai:

| Environment | `COOKIE_SECURE` Value | Cookie Behavior |
|-------------|----------------------|-----------------|
| **Production** (HTTPS) | `true` (ya unset) | `Secure=true; SameSite=None` |
| **Local Dev** (HTTP) | `false` | `Secure=false; SameSite=Lax` |

### Frontend Developer ko Kya Karna Hai?

**Local dev pe:**
- Backend `.env` file mein yeh add karo: `COOKIE_SECURE=false`
- Baaki sab same rahega — login call karo, cookie automatically browser mein store hogi

**Production pe:**
- `COOKIE_SECURE` unset rakho ya `true` karo
- Frontend ko `credentials: 'include'` zaroor dena hai fetch calls mein (neeche dekho)

---

## Endpoints

---

### 1. `POST /api/admin/login`

Admin login endpoint. Successful login pe:
- `admin_session` naam ki **HttpOnly cookie** set hoti hai
- Response body mein `token` bhi milta hai (fallback ke liye)

#### Request

```http
POST /api/admin/login
Content-Type: application/json
```

```json
{
  "email": "admin@livana.com",
  "password": "your-password"
}
```

#### Response — Success `200 OK`

```json
{
  "success": true,
  "message": "Login successful",
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
}
```

> Cookie bhi automatically set hoti hai response mein:
> ```
> Set-Cookie: admin_session=<jwt>; Path=/; HttpOnly; SameSite=None; Secure
> ```
> (Local dev mein: `SameSite=Lax`, `Secure` absent)

#### Response — Failure `401 Unauthorized`

```json
{
  "success": false,
  "message": "Invalid credentials"
}
```

#### Response — Rate Limited `429 Too Many Requests`

```json
{
  "success": false,
  "message": "Too many login attempts. Please try again in 15 minutes."
}
```

> **Rate Limit:** 5 attempts per IP per 15 minutes.

#### Response — Server Error `500 Internal Server Error`

```json
{
  "success": false,
  "message": "Token generation failed"
}
```

---

### 2. `POST /api/admin/logout`

Session cookie clear karta hai. Browser se `admin_session` cookie remove ho jaati hai.

#### Request

```http
POST /api/admin/logout
```

_(No body required)_

#### Response — `200 OK`

```json
{
  "success": true,
  "message": "Logged out successfully"
}
```

> Cookie removal automatically hoti hai response headers mein (negative `max-age` set karta hai server).

---

### 3. `GET /api/admin/me`

Current authenticated admin ki details return karta hai. Yeh endpoint verify karta hai ki session valid hai ya nahi — useful hai page load pe auth check ke liye.

#### Authentication

Server **do tarike se** token accept karta hai (priority order mein):

1. **Cookie** (preferred): `admin_session` cookie automatically browser bhejta hai
2. **Authorization Header** (fallback): `Authorization: Bearer <token>`

#### Request

```http
GET /api/admin/me
```

_(No body required)_

#### Response — Success `200 OK`

```json
{
  "email": "admin@livana.com",
  "role": "admin"
}
```

#### Response — Unauthenticated `401 Unauthorized`

```json
{
  "success": false,
  "message": "No active session"
}
```

```json
{
  "success": false,
  "message": "Invalid or expired session"
}
```

> Token expiry: **24 hours** from login time.

---

## Frontend Integration Examples

### JavaScript / Fetch API

> **Important:** `credentials: 'include'` dena zaroori hai tabhi browser cookie bhejega cross-origin requests mein.

```javascript
// LOGIN
async function adminLogin(email, password) {
  const res = await fetch('https://api.livana.com/api/admin/login', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    credentials: 'include', // ZAROORI HAI
    body: JSON.stringify({ email, password }),
  });

  const data = await res.json();

  if (!res.ok) {
    if (res.status === 429) {
      throw new Error('Too many attempts. Wait 15 minutes.');
    }
    throw new Error(data.message || 'Login failed');
  }

  return data;
}

// LOGOUT
async function adminLogout() {
  await fetch('https://api.livana.com/api/admin/logout', {
    method: 'POST',
    credentials: 'include', // ZAROORI HAI
  });
  // Redirect to login page
}

// CHECK SESSION (page load pe call karo)
async function getAdminMe() {
  const res = await fetch('https://api.livana.com/api/admin/me', {
    method: 'GET',
    credentials: 'include', // ZAROORI HAI
  });

  if (res.status === 401) {
    // Session expired ya nahi hai — login page pe redirect karo
    window.location.href = '/admin/login';
    return null;
  }

  return await res.json(); // { email, role }
}
```

### Axios

```javascript
// Axios instance banaao with credentials
const adminApi = axios.create({
  baseURL: 'https://api.livana.com/api/admin',
  withCredentials: true, // ZAROORI HAI
});

// Login
const loginResponse = await adminApi.post('/login', { email, password });

// Logout
await adminApi.post('/logout');

// Me check
const meResponse = await adminApi.get('/me');
```

### Interceptor — Auto Redirect on 401

```javascript
adminApi.interceptors.response.use(
  (response) => response,
  (error) => {
    if (error.response?.status === 401) {
      window.location.href = '/admin/login';
    }
    return Promise.reject(error);
  }
);
```

---

## JWT Token Details

Token response body mein bhi milta hai (`token` field). Yeh useful hai agar cookie-based approach kisi reason se kaam na kare.

**Token payload structure (decoded):**

```json
{
  "sub": "admin@livana.com",
  "role": "admin",
  "exp": 1753920000
}
```

| Field | Type | Description |
|-------|------|-------------|
| `sub` | string | Admin email address |
| `role` | string | Always `"admin"` |
| `exp` | number | Unix timestamp — expiry time (24 hours after login) |

**Token expiry:** 24 hours. Expiry ke baad `/me` call `401` return karega — user ko re-login karna hoga.

---

## Error Reference

| HTTP Status | `message` | Frontend Action |
|-------------|-----------|-----------------|
| `200` | `"Login successful"` | Dashboard pe redirect karo |
| `200` | `"Logged out successfully"` | Login page pe redirect karo |
| `200` | `{ email, role }` | Session valid, proceed |
| `401` | `"Invalid credentials"` | Error show karo, retry allow karo |
| `401` | `"No active session"` | Login page pe redirect karo |
| `401` | `"Invalid or expired session"` | Token expired, re-login |
| `429` | `"Too many login attempts..."` | 15 min countdown show karo |
| `500` | `"Token generation failed"` | Server error, backend team ko batao |

---

## Local Development Checklist

- [ ] Backend `.env` mein `COOKIE_SECURE=false` add karo
- [ ] Frontend fetch calls mein `credentials: 'include'` lagao
- [ ] Axios use kar rahe ho toh `withCredentials: true` lagao
- [ ] Browser DevTools → Application → Cookies mein `admin_session` check karo after login
- [ ] CORS settings mein frontend origin allow hai (`http://localhost:3000` etc.)

---

## Security Notes (For Context)

- Cookie `HttpOnly` hai — JavaScript se access nahi hoti (XSS protection)
- Production mein `Secure` flag hai — sirf HTTPS pe bhejti hai
- Rate limiting: **5 attempts per IP per 15 minutes** (brute force protection)
- Password bcrypt se hashed hai with cost factor >= 12
- JWT secret server-side env variable hai
