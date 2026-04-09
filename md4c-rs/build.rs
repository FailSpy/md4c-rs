use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let vendor = PathBuf::from(&manifest_dir).join("vendor");

    let mut build = cc::Build::new();
    build
        .file(vendor.join("md4c.c"))
        .include(&vendor)
        .define("MD4C_USE_UTF8", None)
        .warnings(false)
        .opt_level(3);

    #[cfg(feature = "html")]
    {
        build.file(vendor.join("md4c-html.c"));
        build.file(vendor.join("entity.c"));
    }

    build.compile("md4c");

    println!("cargo:rerun-if-changed=vendor/md4c.c");
    println!("cargo:rerun-if-changed=vendor/md4c.h");
    println!("cargo:rerun-if-changed=vendor/md4c-html.c");
    println!("cargo:rerun-if-changed=vendor/md4c-html.h");
    println!("cargo:rerun-if-changed=vendor/entity.c");
    println!("cargo:rerun-if-changed=vendor/entity.h");
}
