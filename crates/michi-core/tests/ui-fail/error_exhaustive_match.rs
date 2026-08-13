fn main() {
    let e = michi_core::Error::InvalidInput("x".to_string());
    match e {
        michi_core::Error::InvalidInput(_) => {}
        michi_core::Error::NotFound(_) => {}
        michi_core::Error::Domain(_) => {}
    }
}
