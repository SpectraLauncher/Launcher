use std::collections::HashMap;

fn load_env_file() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Ok(contents) = std::fs::read_to_string(".env") else { return map };
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let mut v = v.trim();
            if v.len() >= 2
                && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')))
            {
                v = &v[1..v.len() - 1];
            }
            map.insert(k.trim().to_string(), v.to_string());
        }
    }
    map
}

fn main() {
    let file_env = load_env_file();
    let get = |key: &str| std::env::var(key).ok().filter(|s| !s.is_empty())
        .or_else(|| file_env.get(key).cloned())
        .unwrap_or_default();

    println!("cargo:rustc-env=DISCORD_CLIENT_ID={}", get("DISCORD_CLIENT_ID"));
    println!("cargo:rerun-if-changed=.env");

    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let mut windows = tauri_build::WindowsAttributes::new();
    if msvc {
        let manifest = std::env::current_dir()
            .expect("build directory")
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    }

    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run tauri-build");
}
