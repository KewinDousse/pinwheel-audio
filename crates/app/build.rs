fn main() {
    // embed-resource does not track the .rc file itself, so editing it alone
    // would leave stale resources in the executable.
    println!("cargo:rerun-if-changed=res");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("res/pinwheel.rc", embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}
