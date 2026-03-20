 Vibe Matchmaking API (Module 10)

## Overview
The Vibe system allows users to express interest in being roommates for specific properties. Think of it as a "Tinder-style" match request tailored for the roommate-finding experience.

For example: A user posts a property requesting a male roommate. A female user sees the room, needs it urgently, and presses the "Vibe" button. This sends a matching request to the property owner, who can then "Accept" or "Reject" the vibe.

## Tech Stack
- **Database:** PostgreSQL (SQLx)
- **Framework:** Axum (Rust)
- **Constraints Evaluated:** `chk_vibe_status` ensures statuses are strictly lowercase (`pending`, `accepted`, `rejected`).

## API Endpoints

### 1. Send a Vibe
- **Endpoint:** `POST /api/v1/vibes`
- **Auth:** Required (JWT)
- **Payload:**
  ```json
  {
    "target_user_id": "<UUID>",
    "property_id": "<UUID>"
  }
  ```
- **Description:** Sends a match request to the `target_user_id` for the specified `property_id`. The initial status is set to `pending`. Duplicate vibes for the same sender, target, and property update the existing row rather than throwing an error.

### 2. Accept a Vibe
- **Endpoint:** `POST /api/v1/vibes/{vibe_id}/accept`
- **Auth:** Required (JWT)
- **Description:** The user who listed the property (the target of the vibe) accepts the request. Updates the vibe status to `accepted` and updates `updated_at`. Only the `target_user_id` can perform this action.

### 3. Reject a Vibe
- **Endpoint:** `POST /api/v1/vibes/{vibe_id}/reject`
- **Auth:** Required (JWT)
- **Description:** The user who listed the property rejects the request. Updates the vibe status to `rejected` and updates `updated_at`. Only the `target_user_id` can perform this action.

### 4. Get Matches
- **Endpoint:** `GET /api/v1/vibes/matches`
- **Auth:** Required (JWT)
- **Description:** Retrieves all users with whom the authenticated user has an `accepted` vibe (either as the sender or the target). Returns the matched user's basic profile details (first name, last name, image) and the exact `property_id` that triggered the match.

## Database Schema
**Table Name:** `vibes`
- `id`: UUID (Primary Key)
- `sender_id`: UUID (Foreign Key to `users.id`)
- `target_user_id`: UUID (Foreign Key to `users.id`)
- `property_id`: UUID (Foreign Key to `properties.id`)
- `status`: String (`pending`, `accepted`, `rejected`) - protected by `chk_vibe_status`
- `created_at`: TIMESTAMPTZ
- `updated_at`: TIMESTAMPTZ

*Constraint:* `UNIQUE (sender_id, target_user_id, property_id)` prevents spamming the same property from the same user.
