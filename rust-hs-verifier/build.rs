use std::process::Command;
use std::env;

fn main() {
    // Emit git version info
    emit_git_info();
    
    // Emit build timestamp
    emit_build_timestamp();
    
    // Check for required tools
    check_required_tools();
    
    // Emit cargo instructions for rebuild
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
}

fn emit_git_info() {
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    
    let git_branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    
    let git_dirty = Command::new("git")
        .args(["diff", "--quiet"])
        .status()
        .map(|status| !status.success())
        .unwrap_or(true);
    
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);
    println!("cargo:rustc-env=GIT_BRANCH={}", git_branch);
    println!("cargo:rustc-env=GIT_DIRTY={}", if git_dirty { "true" } else { "false" });
}

fn emit_build_timestamp() {
    let timestamp = chrono::Utc::now().to_rfc3339();
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", timestamp);
}

fn check_required_tools() {
    // Check for protoc if needed
    if env::var("CARGO_FEATURE_GRPC").is_ok() {
        match Command::new("protoc").arg("--version").output() {
            Ok(_) => {}
            Err(_) => {
                println!("cargo:warning=protoc not found. Install protobuf-compiler for gRPC support.");
            }
        }
    }
}
