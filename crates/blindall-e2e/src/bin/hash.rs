fn main() { println!("{}", bcrypt::hash("password123", 12).unwrap()); }
