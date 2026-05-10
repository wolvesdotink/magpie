fn main() {
    tauri_build::build();

    // llama-cpp-2 is built as a shared library (dynamic-link feature) to isolate
    // its ggml symbols from whisper-rs-sys's statically-linked ggml.  The resulting
    // dylibs are placed next to the binary by llama-cpp-sys-2's build script, but
    // the binary needs an @rpath entry to find them at runtime.
    //
    // For development: @executable_path covers `cargo run` / `tauri dev`.
    // For distribution: Tauri's bundle config should copy the dylibs into
    // the .app/Contents/Frameworks directory (covered by @executable_path/../Frameworks).
    println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path");
    println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks");
}
