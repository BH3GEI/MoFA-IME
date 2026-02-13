use std::env;
use std::path::PathBuf;

fn main() {
    let cpp_dir = PathBuf::from("cpp");
    let build_dir = cpp_dir.join("build");

    // Tell cargo to look for libraries in the build directory
    println!("cargo:rustc-link-search=native={}", build_dir.display());
    println!("cargo:rustc-link-search=native={}", build_dir.join("llama.cpp/src").display());
    println!("cargo:rustc-link-search=native={}", build_dir.join("llama.cpp/ggml/src").display());
    println!("cargo:rustc-link-search=native={}", build_dir.join("llama.cpp/ggml/src/ggml-metal").display());
    println!("cargo:rustc-link-search=native={}", build_dir.join("llama.cpp/ggml/src/ggml-blas").display());
    println!("cargo:rustc-link-search=native={}", build_dir.join("llama.cpp/ggml/src/ggml-cpu").display());

    // Link our wrapper library (static)
    println!("cargo:rustc-link-lib=static=llm_server");

    // Link llama.cpp libraries (dynamic libraries)
    println!("cargo:rustc-link-lib=dylib=llama");
    println!("cargo:rustc-link-lib=dylib=ggml");
    println!("cargo:rustc-link-lib=dylib=ggml-metal");
    println!("cargo:rustc-link-lib=dylib=ggml-blas");
    println!("cargo:rustc-link-lib=dylib=ggml-cpu");
    println!("cargo:rustc-link-lib=dylib=ggml-base");

    // Link system frameworks (macOS)
    if env::var("TARGET").unwrap().contains("apple") {
        println!("cargo:rustc-link-lib=framework=Metal");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=Accelerate");
        println!("cargo:rustc-link-lib=framework=CoreGraphics");
    }

    // Link C++ standard library
    if env::var("TARGET").unwrap().contains("apple") {
        println!("cargo:rustc-link-lib=c++");
    } else {
        println!("cargo:rustc-link-lib=stdc++");
    }

    // Set rpath for macOS - use @executable_path for relative paths
    if env::var("TARGET").unwrap().contains("apple") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path");
    }

    // Rebuild if C++ files change
    println!("cargo:rerun-if-changed=cpp/llm_server.cpp");
    println!("cargo:rerun-if-changed=cpp/llm_server.h");
}
