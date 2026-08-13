fn main() {
    let c = michi_core::ErrorCode::InvalidInput;
    match c {
        michi_core::ErrorCode::InvalidInput => {}
        michi_core::ErrorCode::NotFound => {}
        michi_core::ErrorCode::Unauthorized => {}
        michi_core::ErrorCode::Forbidden => {}
        michi_core::ErrorCode::Conflict => {}
        michi_core::ErrorCode::RateLimited => {}
        michi_core::ErrorCode::Unavailable => {}
        michi_core::ErrorCode::Timeout => {}
        michi_core::ErrorCode::ExternalFailure => {}
    }
}
