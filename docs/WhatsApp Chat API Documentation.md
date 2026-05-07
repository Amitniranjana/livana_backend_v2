# WhatsApp-Style Chat API Documentation

This document outlines the endpoints and WebSocket payloads for the updated WhatsApp-style chat system. The system now supports realtime delivery receipts, seen receipts, and push notifications utilizing a dedicated local WebSocket server alongside AWS Chime.

---

## 1. WebSocket Connection (Realtime Events)

The WebSocket connection is essential for receiving new messages, delivery receipts, and read receipts in real time. **Clients only need to listen** to this WebSocket. Message sending remains over the REST API.

**Endpoint:**  
`GET wss://<your-api-domain>/api/v1/ws?token=<YOUR_JWT_TOKEN>`

### Event Payloads (Incoming to Frontend)

All payloads received over the WebSocket will be JSON strings containing a `type` field to distinguish the event.

#### A. New Message Received
Triggered when the other user sends a message and you are currently connected.
```json
{
  "type": "new_message",
  "message_id": "c33b4970-1234-5678-abcd-1234567890ab",
  "chat_id": "d11a4970-1234-5678-abcd-1234567890ab",
  "sender_id": "e22c4970-1234-5678-abcd-1234567890ab",
  "content": "Hello there!",  // Or the S3 URL for media
  "message_type": "text",     // Can be "text", "image", or "document"
  "created_at": "2026-05-06T12:00:00Z"
}
```

#### B. Message Delivered (Single to Double Tick)
Triggered instantly when a message you just sent via REST successfully reaches the receiver's active WebSocket connection.
```json
{
  "type": "message_delivered",
  "message_id": "c33b4970-1234-5678-abcd-1234567890ab",
  "delivered_at": "2026-05-06T12:00:02Z"
}
```

#### C. Message Seen (Blue Ticks)
Triggered when the receiver explicitly opens the conversation screen (which calls the `/seen` REST endpoint).
```json
{
  "type": "message_seen",
  "conversation_id": "d11a4970-1234-5678-abcd-1234567890ab",
  "seen_by": "e22c4970-1234-5678-abcd-1234567890ab",
  "seen_at": "2026-05-06T12:05:00Z"
}
```

---

## 2. REST API Updates

### 2.1 Send Message
Remains unchanged in signature, but now automatically triggers the WebSocket delivery system in the background.

**Endpoint:**  
`POST /chat/messages`

**Headers:**
- `Authorization: Bearer <jwt_token>`

**Payload:**
```json
{
  "channel_arn": "arn:aws:chime:.../channel/d11a4970-1234-5678-abcd-1234567890ab",
  "content": "Hello there!"
}
```

**Response:**
```json
{
  "message_id": "c33b4970-1234-5678-abcd-1234567890ab",
  "source": "local"
}
```
> **Note for UI**: The moment you receive `message_id`, mark the message with **One Tick (Sent)** in the UI.

---

### 2.2 Mark Chat as Seen (Trigger Blue Ticks)
Call this endpoint **immediately** when the user opens a specific chat screen. 

**Endpoint:**  
`PATCH /api/v1/chats/{chat_id}/seen`

**Headers:**
- `Authorization: Bearer <jwt_token>`

**Behavior Summary:**
1. Marks all incoming messages in this chat as `status = 'seen'` in the database.
2. Clears all unread in-app offline notifications for this chat.
3. Automatically fires the `message_seen` WebSocket event back to the sender.

**Response:**
```json
{
  "success": true,
  "message": "Chat marked as seen"
}
```

---

### 2.3 Upload Chat Media
Upload an image or document. The backend automatically saves it as a message and pushes it through the WebSocket delivery system.

**Endpoint:**  
`POST /api/v1/chats/upload`

**Headers:**
- `Authorization: Bearer <jwt_token>`
- `Content-Type: multipart/form-data`

**Form-Data Fields:**
- `file`: (Binary File) Required. The image or document (Max 5MB).
- `chat_id`: (String) Required. The UUID of the chat.

**Response:**
```json
{
  "success": true,
  "message": "Media uploaded and message created",
  "file_url": "/uploads/chat/e22c_uuid_image.png",
  "file_type": "image",  // or "document"
  "message_id": "c33b4970-1234-5678-abcd-1234567890ab"
}
```

---

## 3. UI Implementation Guide (WhatsApp Flow)

1. **App Launch**: 
   - Connect to the WebSocket: `wss://<domain>/api/v1/ws?token=<token>`.
2. **Sending a Message**: 
   - User types "Hi".
   - UI shows clock/spinner.
   - Call `POST /chat/messages`.
   - On `200 OK`, get `message_id` and change UI to **Single Tick ✓** (Sent).
3. **Delivery Tracking**: 
   - Wait for WebSocket event `"type": "message_delivered"` with matching `message_id`.
   - Change UI to **Double Tick ✓✓** (Delivered).
4. **Read Tracking**: 
   - Wait for WebSocket event `"type": "message_seen"`.
   - Change UI to **Blue Double Ticks ✓✓** for all previously sent messages in that `conversation_id`.
5. **Receiving a Message**:
   - If user has chat open, you receive `"type": "new_message"` over WebSocket. Immediately call `PATCH /api/v1/chats/{chat_id}/seen` so the sender gets the blue ticks.
   - If user is on a different screen, show an in-app banner/badge using the `"type": "new_message"` payload.
6. **Opening a Chat Screen**:
   - Immediately call `PATCH /api/v1/chats/{chat_id}/seen` on mount.
