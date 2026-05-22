# Frontend Integration Guide: Property Sharing & Deep Linking

This guide outlines everything the frontend (Flutter) team needs to know to implement the **Property Sharing** feature. The backend now supports generating a dynamic HTML page that handles OS-level redirections, app store fallbacks, and WhatsApp/iMessage rich link previews.

---

## 1. How to Share a Property (From the App)

When a user taps the "Share" button on a property listing, you do **not** need to hit an API to generate a link. You simply construct the public URL using the property's UUID and pass it to the native share sheet.

### Constructing the URL
The base URL for sharing a property is:
```text
http://<API_BASE_URL>/share/property/<PROPERTY_ID>
```

### Example using `share_plus` (Flutter)
```dart
import 'package:share_plus/share_plus.dart';

void shareProperty(String propertyId, String propertyTitle) {
  // Replace API_BASE_URL with your actual production/staging domain
  final String shareUrl = 'http://13.216.208.31:9090/share/property/$propertyId';
  
  Share.share(
    'Check out this property on Livana Eco: $shareUrl',
    subject: propertyTitle, // Used for email subjects
  );
}
```

> [!TIP]
> **Rich Previews:** You don't need to do anything extra for WhatsApp, iMessage, or Telegram. The backend automatically injects Open Graph (`og:`) meta tags into the HTML response (including the primary image, price, and location) so link previews will look beautiful automatically.

---

## 2. Handling Incoming Deep Links (Into the App)

When a user receives the link and taps it, the backend HTML page will automatically attempt to open the app using a custom URL scheme: `livanaeco://`. 

If the app is installed, the OS will open the app with the following deep link format:
```text
livanaeco://property/<PROPERTY_ID>
```

### Flutter Integration (using `uni_links` or `go_router`)

You need to ensure your app is configured to listen to the custom scheme `livanaeco`. 

#### Android Setup (`AndroidManifest.xml`)
Ensure your `<activity>` has the appropriate intent filter:
```xml
<intent-filter android:autoVerify="true">
    <action android:name="android.intent.action.VIEW" />
    <category android:name="android.intent.category.DEFAULT" />
    <category android:name="android.intent.category.BROWSABLE" />
    <!-- Accepts URIs that begin with "livanaeco://property" -->
    <data android:scheme="livanaeco" android:host="property" />
</intent-filter>
```

#### iOS Setup (`Info.plist`)
Ensure your URL Types are configured:
```xml
<key>CFBundleURLTypes</key>
<array>
  <dict>
    <key>CFBundleURLName</key>
    <string>com.LiveInBuddy.livein</string>
    <key>CFBundleURLSchemes</key>
    <array>
      <string>livanaeco</string>
    </array>
  </dict>
</array>
```

#### Handling the Link in Dart
When the app opens from a deep link, extract the UUID and navigate to the Property Details screen.
```dart
import 'package:uni_links/uni_links.dart';

Future<void> initDeepLinks() async {
  // 1. App was completely closed, launched from link
  try {
    final initialUri = await getInitialUri();
    if (initialUri != null) {
      _handleDeepLink(initialUri);
    }
  } on FormatException {
    // Handle exception
  }

  // 2. App was in background, brought to foreground via link
  uriLinkStream.listen((Uri? uri) {
    if (uri != null) {
      _handleDeepLink(uri);
    }
  }, onError: (err) {
    // Handle exception
  });
}

void _handleDeepLink(Uri uri) {
  if (uri.scheme == 'livanaeco' && uri.host == 'property') {
    // The path will be '/<PROPERTY_ID>'
    final propertyId = uri.pathSegments.isNotEmpty ? uri.pathSegments.first : null;
    
    if (propertyId != null) {
      // Navigate to your property details screen
      // e.g., navigatorKey.currentState?.pushNamed('/property', arguments: propertyId);
      print("Opened Property UUID: $propertyId");
    }
  }
}
```

---

## 3. Fallback Behavior (What happens if the app isn't installed?)

You don't need to handle app store fallbacks on the frontend. The backend HTML page takes care of this automatically:
- **Android:** The Intent URI includes `S.browser_fallback_url` pointing directly to the Google Play Store.
- **iOS:** The page sets a 1.5-second timeout. If the app fails to open within that window, JS redirects the user to the Apple App Store.
- **Desktop:** Users see a clean landing page asking them to open the link on a mobile device.

### Summary of Responsibilities
* **Backend:** Serves the HTML page, populates OG tags for link previews, handles OS-specific redirect logic and App Store fallbacks.
* **Frontend (Sender):** Constructs the `http://.../share/property/id` URL and invokes the native share sheet.
* **Frontend (Receiver):** Listens for the `livanaeco://property/id` URL scheme and navigates to the Property Detail screen.
