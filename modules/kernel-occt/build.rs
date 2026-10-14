use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=src/ffi.rs");
    println!("cargo:rerun-if-changed=cpp/nominal_thread.h");
    println!("cargo:rerun-if-changed=cpp/nominal_thread.cpp");
    println!("cargo:rerun-if-changed=cpp/variable_fillet.h");
    println!("cargo:rerun-if-changed=cpp/variable_fillet.cpp");
    println!("cargo:rerun-if-env-changed=OCCT_ROOT");
    if env::var_os("CARGO_FEATURE_OCCT").is_none() {
        return;
    }
    // The build dependency on cadrum resolves/validates this same OCCT root
    // before this bridge compiles. No second download or ABI selection.
    let target = env::var("TARGET").expect("Cargo supplies TARGET");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    let above = out.ancestors().nth(4).expect("Cargo OUT_DIR layout");
    let target_dir = if above
        .file_name()
        .is_some_and(|name| name == target.as_str())
    {
        above.parent().expect("target triple parent")
    } else {
        above
    };
    let root = env::var_os("OCCT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| target_dir.join(format!("cadrum-occt-v800-{target}")));
    let include = [
        root.join("include/opencascade"),
        root.join("inc"),
        root.join("include"),
    ]
    .into_iter()
    .find(|path| path.join("BRepFilletAPI_MakeFillet.hxx").exists())
    .expect("cadrum must prepare OCCT 8.0 headers before the bridge compiles");
    cxx_build::bridge("src/ffi.rs")
        .file("cpp/variable_fillet.cpp")
        .file("cpp/nominal_thread.cpp")
        .include(".")
        .include(include)
        .std("c++17")
        .define("_USE_MATH_DEFINES", None)
        .compile("musubicad_variable_fillet");
}
