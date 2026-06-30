use argon2::{password_hash::{rand_core::OsRng, SaltString}, Argon2, PasswordHasher};

fn main() {
    let password = "V143pandey@";
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt).unwrap().to_string();
    println!("{}", password_hash);
}
