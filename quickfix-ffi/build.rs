use std::{env, fs, path::Path, process::Command};

use cmake::Config;
use fs_extra::dir::CopyOptions;

fn have_feature(flag: &str) -> bool {
    env::var(format!(
        "CARGO_FEATURE_{}",
        flag.replace('-', "_").to_uppercase()
    ))
    .is_ok()
}

fn read_cmake_opt(flag: &str) -> &'static str {
    if have_feature(flag) {
        "ON"
    } else {
        "OFF"
    }
}

fn env_var(name: &str) -> Option<String> {
    env::var(name).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn split_flags(value: &str) -> impl Iterator<Item = String> + '_ {
    value
        .split_whitespace()
        .map(str::trim)
        .filter(|flag| !flag.is_empty())
        .map(ToOwned::to_owned)
}

fn get_compiler_launcher() -> Option<&'static str> {
    if let Ok(res) = Command::new("sccache").arg("--version").status() {
        if res.success() {
            return Some("sccache");
        }
    }
    if let Ok(res) = Command::new("ccache").arg("--version").status() {
        if res.success() {
            return Some("ccache");
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PgoMode {
    #[default]
    Off,
    Generate,
    Use,
}

impl PgoMode {
    fn from_env() -> Self {
        match env_var("QUICKFIX_PGO_MODE")
            .unwrap_or_else(|| "off".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "off" | "none" => Self::Off,
            "gen" | "generate" => Self::Generate,
            "use" => Self::Use,
            other => panic!(
                "Unsupported QUICKFIX_PGO_MODE='{other}'. Expected one of: off, generate, use"
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LtoMode {
    Thin,
    Full,
}

impl LtoMode {
    fn from_env() -> Option<Self> {
        match env_var("QUICKFIX_LTO")
            .unwrap_or_else(|| "off".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "off" | "none" | "0" => None,
            "thin" => Some(Self::Thin),
            "full" | "on" | "1" => Some(Self::Full),
            other => panic!("Unsupported QUICKFIX_LTO='{other}'. Expected one of: off, thin, full"),
        }
    }

    fn as_flag(self) -> &'static str {
        match self {
            Self::Thin => "-flto=thin",
            Self::Full => "-flto",
        }
    }
}

#[derive(Debug, Default)]
struct PerfFlags {
    c_flags: Vec<String>,
    cxx_flags: Vec<String>,
    enable_ipo: bool,
}

impl PerfFlags {
    fn from_env(out_dir: &str) -> Self {
        let mut flags = Self::default();

        let pgo_mode = PgoMode::from_env();
        if pgo_mode != PgoMode::Off {
            let pgo_dir =
                env_var("QUICKFIX_PGO_DIR").unwrap_or_else(|| format!("{out_dir}/quickfix-pgo"));
            match pgo_mode {
                PgoMode::Generate => {
                    let flag = format!("-fprofile-generate={pgo_dir}");
                    flags.c_flags.push(flag.clone());
                    flags.cxx_flags.push(flag);
                }
                PgoMode::Use => {
                    let flag = format!("-fprofile-use={pgo_dir}");
                    flags.c_flags.push(flag.clone());
                    flags.cxx_flags.push(flag);
                    flags.c_flags.push("-fprofile-correction".to_string());
                    flags.cxx_flags.push("-fprofile-correction".to_string());
                }
                PgoMode::Off => {}
            }
        }

        if let Some(sample_profile) =
            env_var("QUICKFIX_SAMPLE_PROFILE").or_else(|| env_var("QUICKFIX_AUTOFDO_PROFILE"))
        {
            let flag = format!("-fprofile-sample-use={sample_profile}");
            flags.c_flags.push(flag.clone());
            flags.cxx_flags.push(flag);
        }

        if let Some(lto_mode) = LtoMode::from_env() {
            let flag = lto_mode.as_flag().to_string();
            flags.c_flags.push(flag.clone());
            flags.cxx_flags.push(flag);
            flags.enable_ipo = true;
        }

        if let Some(extra_c_flags) = env_var("QUICKFIX_EXTRA_CFLAGS") {
            flags.c_flags.extend(split_flags(&extra_c_flags));
        }
        if let Some(extra_cxx_flags) = env_var("QUICKFIX_EXTRA_CXXFLAGS") {
            flags.cxx_flags.extend(split_flags(&extra_cxx_flags));
        }

        flags
    }

    fn apply_to(&self, config: &mut Config) {
        if self.enable_ipo {
            config.define("CMAKE_INTERPROCEDURAL_OPTIMIZATION", "ON");
        }
        for flag in &self.c_flags {
            config.cflag(flag);
        }
        for flag in &self.cxx_flags {
            config.cxxflag(flag);
        }
    }
}

fn declare_rerun_envs() {
    for key in [
        "QUICKFIX_PGO_MODE",
        "QUICKFIX_PGO_DIR",
        "QUICKFIX_SAMPLE_PROFILE",
        "QUICKFIX_AUTOFDO_PROFILE",
        "QUICKFIX_LTO",
        "QUICKFIX_EXTRA_CFLAGS",
        "QUICKFIX_EXTRA_CXXFLAGS",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }
}

fn main() {
    let out_dir = env::var("OUT_DIR").expect("Missing OUT_DIR");
    let target_os = TargetOs::from_env();

    if have_feature("build-with-io-uring") && target_os != TargetOs::Linux {
        panic!("Feature `build-with-io-uring` is only supported on Linux targets");
    }

    // Make sure sub-repositories are correctly init
    update_sub_repositories();

    // Tell Cargo that if the given file changes, to rerun this build script.
    println!("cargo:rerun-if-changed=./CMakeLists.txt");
    println!("cargo:rerun-if-changed=./libquickfix");
    println!("cargo:rerun-if-changed=./quickfix-bind");
    declare_rerun_envs();

    // Clone libquickfix to OUT_DIR because it modify itself when building
    let libquickfix_build_dir = Path::new(&out_dir).join("libquickfix");

    let _ = fs::remove_dir_all(&libquickfix_build_dir);
    fs_extra::copy_items(&["./libquickfix"], &out_dir, &CopyOptions::default())
        .expect("Fail to copy libquickfix");

    let perf_flags = PerfFlags::from_env(&out_dir);

    // Build quickfix as a static library
    let mut quickfix_cmake_config = Config::new(libquickfix_build_dir);
    quickfix_cmake_config
        .define("CMAKE_POLICY_VERSION_MINIMUM", "3.10")
        .define("HAVE_SSL", read_cmake_opt("build-with-ssl"))
        .define("HAVE_MYSQL", read_cmake_opt("build-with-mysql"))
        .define("HAVE_POSTGRESQL", read_cmake_opt("build-with-postgres"))
        .define("HAVE_IO_URING", read_cmake_opt("build-with-io-uring"))
        .define("HAVE_PYTHON", "OFF")
        .define("HAVE_PYTHON3", "OFF")
        .define("QUICKFIX_SHARED_LIBS", "OFF")
        .define("QUICKFIX_EXAMPLES", "OFF")
        .define("QUICKFIX_TESTS", "OFF")
        // Always compile libquickfix in release mode.
        // We are not here to debug this library.
        .profile("RelWithDebInfo");
    perf_flags.apply_to(&mut quickfix_cmake_config);

    if let Some(compiler_launcher) = get_compiler_launcher() {
        quickfix_cmake_config
            .define("CMAKE_C_COMPILER_LAUNCHER", compiler_launcher)
            .define("CMAKE_CXX_COMPILER_LAUNCHER", compiler_launcher);
    }

    let quickfix_dst = quickfix_cmake_config.build();
    let quickfix_include_path = format!("{}/include", quickfix_dst.display());
    let quickfix_lib_path = format!("{}/lib", quickfix_dst.display());

    // Build quickfix C bind also as a static library.
    env::set_var("CMAKE_LIBRARY_PATH", [quickfix_lib_path].join(";"));

    let mut quickfix_bind_cmake_config = Config::new(".");
    quickfix_bind_cmake_config
        .cflag(format!("-I{quickfix_include_path}"))
        .cxxflag(format!("-I{quickfix_include_path}"))
        .define("QUICKFIX_BIND_EXAMPLES", "OFF")
        .define("HAVE_SSL", read_cmake_opt("build-with-ssl"))
        .define("HAVE_MYSQL", read_cmake_opt("build-with-mysql"))
        .define("HAVE_POSTGRESQL", read_cmake_opt("build-with-postgres"));
    perf_flags.apply_to(&mut quickfix_bind_cmake_config);

    if let Some(compiler_launcher) = get_compiler_launcher() {
        quickfix_bind_cmake_config
            .define("CMAKE_C_COMPILER_LAUNCHER", compiler_launcher)
            .define("CMAKE_CXX_COMPILER_LAUNCHER", compiler_launcher);
    }

    let quickfix_bind_dst = quickfix_bind_cmake_config.build();

    // Configure rustc.
    println!(
        "cargo:rustc-link-search=native={}/lib",
        quickfix_dst.display()
    );
    println!(
        "cargo:rustc-link-search=native={}/lib",
        quickfix_bind_dst.display()
    );

    // ⚠️ NOTE: libquickfix as a different name on windows with debug profile.
    println!("cargo:rustc-link-lib=static=quickfix");
    println!("cargo:rustc-link-lib=static=quickfixbind");

    // Lib std C++ is only available on UNIX platform.
    if let Some(lib_cpp_name) = target_os.lib_std_cpp_name() {
        println!("cargo:rustc-link-lib={lib_cpp_name}");
    }

    // Link with external libraries if needed.
    if have_feature("build-with-ssl") {
        println!("cargo:rustc-link-lib=ssl");
        println!("cargo:rustc-link-lib=crypto");
    }
    if have_feature("build-with-mysql") {
        println!("cargo:rustc-link-lib=mysqlclient");
    }
    if have_feature("build-with-postgres") {
        println!("cargo:rustc-link-lib=pq");
    }
    if have_feature("build-with-io-uring") {
        println!("cargo:rustc-link-lib=uring");
    }
}

fn update_sub_repositories() {
    if Path::new("libquickfix/LICENSE").exists() {
        return;
    }

    if !Command::new("git")
        .args(["submodule", "update", "--init", "--recursive"])
        .current_dir("..")
        .status()
        .expect("Fail to get command status")
        .success()
    {
        panic!("Fail to update sub repo");
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum TargetOs {
    Windows,
    Linux,
    Other,
}

impl TargetOs {
    fn from_env() -> Self {
        match env::var("CARGO_CFG_TARGET_OS").as_deref() {
            Ok("windows") => Self::Windows,
            Ok("linux") => Self::Linux,
            _ => Self::Other,
        }
    }

    fn lib_std_cpp_name(&self) -> Option<&'static str> {
        match self {
            Self::Windows => None,
            Self::Linux => Some("stdc++"),
            Self::Other => Some("c++"),
        }
    }
}
