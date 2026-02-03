//! Build script for rack-vst3-gui
//! Compiles the C++ VST3 GUI wrapper

use std::env;
use std::path::PathBuf;

fn main() {
    // Find the VST3 SDK from rack's build output
    let out_dir = env::var("OUT_DIR").unwrap();
    let target_dir = PathBuf::from(&out_dir)
        .ancestors()
        .nth(3)  // Go up to target/debug/build
        .unwrap()
        .to_path_buf();

    // Look for rack's VST3 SDK build
    let vst3_sdk = find_vst3_sdk(&target_dir);

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .file("cpp/vst3_gui.cpp")
        .include("cpp");

    if let Some(sdk_path) = vst3_sdk {
        println!("cargo:warning=Found VST3 SDK at: {}", sdk_path.display());
        build.include(&sdk_path);
        build.include(sdk_path.join("pluginterfaces"));
        build.include(sdk_path.join("public.sdk"));
        build.include(sdk_path.join("public.sdk/source/vst"));

        // Add VST3 SDK hosting sources
        let hosting_dir = sdk_path.join("public.sdk/source/vst/hosting");
        if hosting_dir.exists() {
            // Core hosting sources
            let module_src = hosting_dir.join("module.cpp");
            let module_linux_src = hosting_dir.join("module_linux.cpp");
            let hostclasses_src = hosting_dir.join("hostclasses.cpp");
            let plugprovider_src = hosting_dir.join("plugprovider.cpp");

            if module_src.exists() {
                build.file(&module_src);
                println!("cargo:warning=Adding module.cpp");
            }
            #[cfg(target_os = "linux")]
            if module_linux_src.exists() {
                build.file(&module_linux_src);
                println!("cargo:warning=Adding module_linux.cpp");
            }
            if hostclasses_src.exists() {
                build.file(&hostclasses_src);
                println!("cargo:warning=Adding hostclasses.cpp");
            }
            if plugprovider_src.exists() {
                build.file(&plugprovider_src);
                println!("cargo:warning=Adding plugprovider.cpp");
            }

            // Base sources needed for FUID, etc.
            let base_dir = sdk_path.join("base/source");
            if base_dir.exists() {
                for entry in std::fs::read_dir(&base_dir).unwrap() {
                    let entry = entry.unwrap();
                    let path = entry.path();
                    if path.extension().map_or(false, |e| e == "cpp") {
                        build.file(&path);
                    }
                }
            }

            // Pluginterfaces base
            let pi_base_dir = sdk_path.join("pluginterfaces/base");
            let funknown_src = pi_base_dir.join("funknown.cpp");
            if funknown_src.exists() {
                build.file(&funknown_src);
            }

            // Common sources (MemoryStream, etc.)
            let common_dir = sdk_path.join("public.sdk/source/common");
            let memorystream_src = common_dir.join("memorystream.cpp");
            if memorystream_src.exists() {
                build.file(&memorystream_src);
                println!("cargo:warning=Adding memorystream.cpp");
            }
        }

        // Define build mode for VST3 SDK
        let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
        if profile == "release" {
            build.define("RELEASE", "1");
            build.define("NDEBUG", "1");
        } else {
            build.define("DEVELOPMENT", "1");
            build.define("_DEBUG", "1");
        }

        // Define VST3 platform
        #[cfg(target_os = "linux")]
        {
            build.define("SMTG_OS_LINUX", "1");
            build.flag("-fPIC");
            build.flag("-Wno-extra");
            build.flag("-Wno-unused-parameter");
            // Link dl for dlopen
            println!("cargo:rustc-link-lib=dl");
        }
        #[cfg(target_os = "macos")]
        build.define("SMTG_OS_MACOS", "1");
        #[cfg(target_os = "windows")]
        build.define("SMTG_OS_WINDOWS", "1");

        // Link against rack's VST3 library if available
        let lib_dir = target_dir.join("deps");
        if lib_dir.exists() {
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
        }
    } else {
        println!("cargo:warning=VST3 SDK not found, building stub");
        // Build a stub that returns errors
        build.define("VST3_GUI_STUB", "1");
    }

    build.compile("vst3_gui");

    // Link C++ standard library
    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-lib=stdc++");
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=c++");

    println!("cargo:rerun-if-changed=cpp/vst3_gui.cpp");
    println!("cargo:rerun-if-changed=cpp/vst3_gui.h");
}

fn find_vst3_sdk(target_dir: &PathBuf) -> Option<PathBuf> {
    // Look in rack's build output
    let build_dir = target_dir.join("build");
    if !build_dir.exists() {
        return None;
    }

    // Find rack-* directory
    for entry in std::fs::read_dir(&build_dir).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("rack-") {
            let sdk_path = entry.path().join("out/vst3sdk");
            if sdk_path.exists() {
                return Some(sdk_path);
            }
        }
    }

    None
}
