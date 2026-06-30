import smtplib
import os

try:
    server = smtplib.SMTP("smtp.gmail.com", 587)
    server.starttls()
    server.login("thelive.inbuddy@gmail.com", "umixdalimqsvdjas")
    print("SUCCESS: Logged in to SMTP successfully!")
    server.quit()
except Exception as e:
    print("FAILED:", str(e))
