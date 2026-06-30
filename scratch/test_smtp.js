const nodemailer = require("nodemailer");

async function testSmtp() {
  try {
    let transporter = nodemailer.createTransport({
      host: "smtp.gmail.com",
      port: 587,
      secure: false, // true for 465, false for other ports
      auth: {
        user: "thelive.inbuddy@gmail.com",
        pass: "umixdalimqsvdjas",
      },
    });

    let info = await transporter.verify();
    console.log("SUCCESS: Logged in to SMTP successfully!");
  } catch (e) {
    console.log("FAILED:", e.message);
  }
}

testSmtp();
