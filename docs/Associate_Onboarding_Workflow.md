# Associate Registration & Onboarding Workflow

**Phase 1: Account Creation (Sign Up)**
* **Role Selection:** User sees two options on the screen: `User` or `Associate`. The user selects `Associate` (Property owner/Broker/Care services).
* **Input Data:**
  * **Full Name:** User enters their name (e.g., Jatin Krishna).
  * **Mobile Number:** Valid 10-digit number with +91 country code.
  * **Email (Optional):** Option to enter an email address.
  * **Gender:** Male/Female/Other selection.
  * **Password:** Set and confirm password (Eye icon for visibility).
  * **Terms & Conditions:** Accept policies by clicking the checkbox.
* **Action:** Click on the "Sign Up" button.

**Phase 2: Verification (OTP)**
* **OTP Generation:** System sends a 6-digit OTP to the registered mobile number.
* **Input:** User enters the OTP.
* **UI Issue (Note for Devs):** There is a "Right overflowed by 48 pixels" error on the screen that needs to be fixed.
* **Action:** Click "Verify OTP" to complete verification.

**Phase 3: Role Specification & Professional Login**
* **Sign In:** User logs in using their new credentials (Email/Phone + Password).
* **Profile Category:** Post-login, the system asks:
  * **Owner/Developer/Broker:** For those who manage/rent properties.
  * **Carecrew:** For those providing services or maintenance.
* **Incentive Hook:** A "You're missing out!" pop-up is shown to push the user to claim rewards and exclusive offers.

**Phase 4: KYC & Compliance (Step-by-Step)**
*User has to complete 3 steps in this phase:*
* **Step 1: Personal Details**
  * Sub-role selection: Property Owner or Property Broker.
  * Fields: First Name, Last Name, Mobile Number, Email-ID, and Date of Birth.
* **Step 2: Address Details** (Part of the flow).
* **Step 3: Identity Proof** (Part of the flow).
* **Note:** A "Skip" option is provided at the top right.

**Phase 5: Associate Dashboard (Final Landing)**
*After KYC or onboarding, the user lands on the Livana Associate Dashboard:*
* **Property Navigation:** Residential, Commercial, Acres, etc.
* **Analytics At-a-glance:**
  * **Total Properties:** Count of properties.
  * **Chats:** Active messages.
  * **Earnings:** Total revenue (e.g., Rs. 25,001).
  * **Views:** Property visibility (e.g., 15,000).
* **Location:** The dashboard automatically attempts to fetch the location.

---

### Key Technical Observations for Developers
* **Validation:** Real-time validation required on every input field (Phone number length, Email format).
* **State Management:** Dashboard layout changes entirely based on the selected Role (Associate vs User).
* **UI/UX Fix:** Must handle the overflow error on the OTP screen using responsive design.
