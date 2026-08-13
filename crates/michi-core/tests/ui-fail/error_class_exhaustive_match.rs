fn main() {
    let c = michi_core::ErrorClass::User;
    match c {
        michi_core::ErrorClass::User => {}
        michi_core::ErrorClass::Internal => {}
        michi_core::ErrorClass::Transient => {}
    }
}
