# Update: Area-Based Notifications & Community Features

Hi Frontend Team,

The backend implementation for **Area-Based Notifications** and **Community Features** has been successfully merged and pushed to the `serviceProvider` branch. 

Here is the updated API documentation outlining the new and modified features for integration.

---

## 1. Area-Based Notifications
Notifications are now fully automated based on the user's selected area.

### New User Profile Field
- **Endpoint:** Any user creation/update endpoints
- **New Field:** `selected_area` (String)
- **Behavior:** You can now provide `selected_area` (e.g., city name or precise location) for a user. Whenever a new listing or expo event is created in that specific area, the backend will automatically dispatch an in-app notification to the user.

### Fetching Notifications
- **Endpoint:** `GET /api/v1/notifications` (Requires Authentication)
- **Description:** Returns a list of notifications for the user, including the newly generated area-based triggers (`type = "SYSTEM"` or `"EXPO"`).

### Marking Notification as Read
- **Endpoint:** `PATCH /api/v1/notifications/{notification_id}/read` (Requires Authentication)
- **Description:** Marks a specific notification as read.

---

## 2. Community Features (Module 8 Updates)

We have extended the community endpoints to support editing, deleting, and fetching a global feed.

### 2.1 Get Community Feed
- **Endpoint:** `GET /api/v1/communities/feed`
- **Method:** `GET`
- **Auth Required:** Yes
- **Description:** Fetches a paginated feed of the latest posts strictly from communities that the currently authenticated user has joined.
- **Response Format:**
  ```json
  {
    "success": true,
    "message": "Community feed fetched successfully",
    "data": [
      {
        "post_id": "uuid",
        "community_id": "uuid",
        "author_id": "uuid",
        "content": "Post content string",
        "created_at": "timestamp"
      }
    ]
  }
  ```

### 2.2 Create Community Post
- **Endpoint:** `POST /api/v1/communities/{community_id}/posts`
- **Auth Required:** Yes (Must be a member of the community)
- **Update:** Upon successful creation, the backend will now **automatically trigger a notification** to all other members of the community notifying them of the new post.

### 2.3 Edit Community Post
- **Endpoint:** `PUT /api/v1/communities/{community_id}/posts/{post_id}`
- **Auth Required:** Yes
- **Payload:**
  ```json
  {
    "content": "Updated post content"
  }
  ```
- **Permissions Update:** Both the **author of the post** and the **creator/admin of the community** are now permitted to edit the post.

### 2.4 Delete Community Post [NEW]
- **Endpoint:** `DELETE /api/v1/communities/{community_id}/posts/{post_id}`
- **Auth Required:** Yes
- **Description:** Deletes a specific post within a community.
- **Permissions:** Both the **author of the post** and the **creator/admin of the community** are permitted to delete the post.
- **Response Format:**
  ```json
  {
    "success": true,
    "message": "Post deleted successfully",
    "data": {}
  }
  ```

### 2.5 Edit Community
- **Endpoint:** `PUT /api/v1/communities/{community_id}`
- **Update:** When a community is updated, a notification is now automatically sent to all its members regarding the update.

---
Let us know if you have any questions or require modifications to the response shapes!
