fn main() {
    let o = michi_toon::ToonOptions::new("t", vec![], vec![]);

    // AC-001b: there is no `new`/`new_unchecked` associated function -- this
    // must fail with E0599 (no function or associated item named `new`
    // found for struct `ToonDocument`).
    let _via_new = michi_toon::ToonDocument::new(&o);
}
