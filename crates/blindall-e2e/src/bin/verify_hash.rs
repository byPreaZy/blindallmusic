fn main() {
    let pw = std::env::args().nth(1).unwrap();
    let h = std::env::args().nth(2).unwrap();
    println!("{}", bcrypt::verify(pw, &h).unwrap());
}
