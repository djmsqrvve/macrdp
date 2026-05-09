fn main() {
    // Link the Accessibility framework for AXIsProcessTrusted and AXIsProcessTrustedWithOptions
    println!("cargo:rustc-link-lib=framework=ApplicationServices");
    println!("cargo:rustc-link-lib=framework=Accessibility");
}
