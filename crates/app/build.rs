//! Embeds the icon, the version information and the manifest. The `.rc` and
//! the manifest in `res/` are templates: the version comes from Cargo, so a
//! release only has to bump it there.

use std::path::Path;

fn main() {
    // Any edit under res/ must rebuild the resources.
    println!("cargo:rerun-if-changed=res");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let env = |name: &str| std::env::var(name).unwrap();
    let out_dir = env("OUT_DIR");
    let res_dir = Path::new(&env("CARGO_MANIFEST_DIR")).join("res");
    let (major, minor, patch) = (
        env("CARGO_PKG_VERSION_MAJOR"),
        env("CARGO_PKG_VERSION_MINOR"),
        env("CARGO_PKG_VERSION_PATCH"),
    );
    // Forward slashes: a backslash starts an escape in .rc strings.
    let slashes = |path: &Path| path.display().to_string().replace('\\', "/");
    let fill = |template: &str| {
        std::fs::read_to_string(res_dir.join(template))
            .unwrap()
            .replace("@VERSION@", &env("CARGO_PKG_VERSION"))
            .replace("@VERSION_COMMAS@", &format!("{major},{minor},{patch},0"))
            .replace("@VERSION_DOTS@", &format!("{major}.{minor}.{patch}.0"))
            .replace("@RES_DIR@", &slashes(&res_dir))
            .replace("@OUT_DIR@", &slashes(Path::new(&out_dir)))
    };
    let manifest = Path::new(&out_dir).join("pinwheel.manifest");
    std::fs::write(&manifest, fill("pinwheel.manifest")).unwrap();
    let rc = Path::new(&out_dir).join("pinwheel.rc");
    std::fs::write(&rc, fill("pinwheel.rc")).unwrap();
    embed_resource::compile(&rc, embed_resource::NONE)
        .manifest_required()
        .unwrap();
}
