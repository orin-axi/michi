fn main() {
    let resp = michi_core::StatusResponse::new("t", "d", vec![]);
    let _ = serde_json::to_string(&resp);
}
