use std::process::Command;
fn main() {
    // Tell Cargo to rerun this build script if HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/tags");

    let output = Command::new("git")
        .args(&["describe", "--tags", "--dirty", "--always"])
        .output()
        .unwrap();
    let git_version = String::from_utf8(output.stdout).unwrap();
    println!("cargo:rustc-env=GIT_VERSION={}", git_version.trim());
}
