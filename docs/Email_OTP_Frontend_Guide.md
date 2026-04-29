# Frontend Guide: Email OTP & Authentication Flows

The backend authentication system has been upgraded to support **Email-based OTPs** alongside the legacy SMS-based OTPs. This guide covers how to integrate these flows on the frontend.

## 🚀 Key Changes
1. **Email Priority:** If both `email` and `phone_no` are provided in a payload, the backend will prioritize sending the OTP via Email.
2. **Purpose Tagging:** OTPs are now tagged with a `purpose` (e.g., `signup`, `forgot_password`) to ensure an OTP generated for one flow cannot be exploited in another.
3. **Forgot Password:** The password reset flow is now **Email-only**. No SMS will be sent for password resets.

---

## 🛠️ Local Development & Testing

### Environment Variables
To fully test email delivery on your local machine, the backend `.env` file must include the following Gmail SMTP credentials:

```env
SMTP_USERNAME=your_gmail@gmail.com
SMTP_PASSWORD=your_gmail_app_password
SMTP_FROM=your_gmail@gmail.com
SMTP_FROM_NAME=Care Connect
```

> **What if you don't have SMTP credentials?**
> You can still test the flow! If SMTP credentials are missing, the backend API will still return a `200 OK` success response. To get the OTP code to input on your frontend UI, simply check the **backend server terminal logs** — the code will be printed there under a `[DEV MODE] INTERCEPT` banner.

---

## 📖 API Endpoints

### 1. Request OTP (Signup / Login)
Triggers an OTP to the user's email (or phone).

- **Endpoint:** `POST /api/auth/send-otp`
- **Payload:**
```json
{
  "email": "user@example.com",
  "purpose": "signup" // Options: "signup", "login", etc.
}
```
*(Note: You can still pass `"phone_no": "+919999999999"` instead of email for legacy SMS flow).*

- **Success Response (200 OK):**
```json
{
  "success": true,
  "message": "OTP sent successfully",
  "data": null
}
```

### 2. Verify OTP
Validates the OTP and returns the authenticated user data and JWT token.

- **Endpoint:** `POST /api/auth/verify-otp`
- **Payload:**
```json
{
  "email": "user@example.com",
  "otp": "123456",
  "purpose": "signup"
}
```

- **Success Response (200 OK):**
```json
{
  "success": true,
  "message": "Email verified successfully",
  "data": {
    "token": "eyJhbGciOiJIUzI1NiIs...",
    "user": {
      "id": "uuid",
      "email": "user@example.com",
      "is_phone_verified": true,
      // ...other user fields
    }
  }
}
```

### 3. Resend OTP
Invalidates any previously unused OTPs and sends a fresh one.

- **Endpoint:** `POST /api/auth/resend-otp`
- **Payload:**
```json
{
  "email": "user@example.com",
  "purpose": "signup"
}
```

### 4. Forgot Password Flow
Generates a reset code and emails it to the user.

- **Endpoint:** `POST /api/auth/send-forgot-password-link`
- **Payload:**
```json
{
  "email": "user@example.com"
}
```
- **Success Response (200 OK):**
```json
{
  "success": true,
  "message": "Reset code generated and sent to email",
  "data": {
    "email": "user@example.com",
    "reset_code_sent": true
  }
}
```

### 5. Reset Password
Verifies the reset code and updates the user's password in one step.

- **Endpoint:** `POST /api/auth/reset-password`
- **Payload:**
```json
{
  "email": "user@example.com",
  "code": "123456",
  "new_password": "NewStrongPassword123!"
}
```
- **Success Response (200 OK):**
```json
{
  "success": true,
  "message": "Password reset successfully",
  "data": {
    "password_updated": true,
    "user_id": "uuid"
  }
}
```

---

## ⚠️ Error Handling

Your frontend should handle the following common HTTP status codes returned by these endpoints:

| Status Code | Reason | Example Backend Message |
|-------------|--------|-------------------------|
| `400 Bad Request` | Missing fields or malformed request. | "Either email or phoneNo must be provided" |
| `401 Unauthorized` | The OTP is wrong or expired. | "Invalid OTP" or "OTP has expired" |
| `404 Not Found` | Trying to verify/reset an unregistered email. | "User not found with this email" |
| `500 Server Error` | Database failure or critical backend issue. | "Failed to store OTP..." |

> **OTP Expiry:** All OTPs are valid for **10 minutes**. Make sure your frontend UI reflects this timer so users know when they need to request a new code.
