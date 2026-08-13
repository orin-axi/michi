fn main() {
    let t = michi_truncate::truncate("hello", 100, "x");
    match t {
        michi_truncate::Truncated { content, original_len, was_truncated, signal } => {
            let _ = (content, original_len, was_truncated, signal);
        }
    }
}
