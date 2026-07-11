use std::process::Command;

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    let apple = target.contains("apple-darwin");

    println!("cargo:rerun-if-env-changed=BFD_BUILD_COMMIT");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    emit_git_rerun_paths();
    let commit = std::env::var("BFD_BUILD_COMMIT")
        .or_else(|_| std::env::var("GITHUB_SHA"))
        .ok()
        .or_else(|| command_output("git", &["rev-parse", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());
    let rustc = std::env::var("RUSTC")
        .ok()
        .and_then(|program| command_output(&program, &["--version"]))
        .unwrap_or_else(|| "unknown".to_string());
    let cargo = std::env::var("CARGO")
        .ok()
        .and_then(|program| command_output(&program, &["--version"]))
        .unwrap_or_else(|| "unknown".to_string());
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=BFD_BUILD_COMMIT={commit}");
    println!("cargo:rustc-env=BFD_BUILD_RUSTC={rustc}");
    println!("cargo:rustc-env=BFD_BUILD_CARGO={cargo}");
    println!("cargo:rustc-env=BFD_BUILD_TARGET={target}");
    println!("cargo:rustc-env=BFD_BUILD_PROFILE={profile}");

    if apple && std::env::var_os("CARGO_FEATURE_METAL_ACCEL").is_some() {
        println!("cargo:rustc-link-lib=framework=Metal");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }

    if apple && std::env::var_os("CARGO_FEATURE_MACOS_VISION").is_some() {
        println!("cargo:rustc-link-lib=framework=Vision");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=CoreGraphics");
    }
}

fn emit_git_rerun_paths() {
    if let Some(head) = command_output("git", &["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={head}");
    }
    if let Some(reference) = command_output("git", &["symbolic-ref", "-q", "HEAD"])
        && let Some(path) = command_output("git", &["rev-parse", "--git-path", &reference])
    {
        println!("cargo:rerun-if-changed={path}");
    }
}

fn command_output(program: &str, arguments: &[&str]) -> Option<String> {
    let output = Command::new(program).args(arguments).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
