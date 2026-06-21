fn main() {
    #[cfg(target_os = "macos")]
    build_ocr_helper();
    tauri_build::build();
}

#[cfg(target_os = "macos")]
fn build_ocr_helper() {
    use std::process::Command;
    println!("cargo:rerun-if-changed=ocr-helper/main.swift");
    let _ = std::fs::create_dir_all("binaries");
    let src = "ocr-helper/main.swift";
    let arm = "binaries/ocr-helper-arm64";
    let x86 = "binaries/ocr-helper-x86_64";
    let out = "binaries/ocr-helper";
    let swift = |target: &str, dst: &str| {
        Command::new("swiftc").args(["-O", "-target", target, "-o", dst, src])
            .status().map(|s| s.success()).unwrap_or(false)
    };
    let a = swift("arm64-apple-macosx12.0", arm);
    let x = swift("x86_64-apple-macosx12.0", x86);
    if a && x {
        let merged = Command::new("xcrun")
            .args(["lipo", "-create", "-output", out, arm, x86])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !merged {
            let _ = Command::new("/usr/bin/lipo")
                .args(["-create", "-output", out, arm, x86])
                .status();
        }
    } else if a {
        let _ = std::fs::copy(arm, out);
    } else if x {
        let _ = std::fs::copy(x86, out);
    } else {
        println!("cargo:warning=swiftc not available; ocr-helper not built (OCR will be disabled)");
    }
}
