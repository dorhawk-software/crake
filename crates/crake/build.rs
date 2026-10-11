//! Windows: crake.exe's icon (crake.rc). Nothing on other systems.

fn main() {
    println!("cargo::rerun-if-changed=crake.rc");
    println!("cargo::rerun-if-changed=../../assets/icons/app/crake.ico");
    embed_resource::compile("crake.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("crake.rc compiles");
}
