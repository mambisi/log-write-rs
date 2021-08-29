use std::cmp::min;

pub fn strncat(dest : &mut String, src : String, n : usize ) {
    dest.push_str(&src[..n])
}


pub fn strnlen<S : AsRef<str>>(src : S, max_len : usize ) -> usize {
    min(src.as_ref().len(), max_len)
}
#[test]
fn test_strncat() {
    let mut hello = "Hello ".to_string();
    let world = "World Micheal";
    strncat(&mut hello, world.to_string(), 5 );
    assert_eq!(hello, "Hello World");
    println!("{}", hello);
}