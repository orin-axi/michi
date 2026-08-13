fn main() {
    let done = michi_resilience::already_done(None);
    match done {
        michi_resilience::AlreadyDone::Yes { result } => {
            let _ = result;
        }
        michi_resilience::AlreadyDone::No => {}
    }
}
