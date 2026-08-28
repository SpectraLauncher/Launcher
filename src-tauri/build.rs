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
    println!("cargo:rustc-env=CURSEFORGE_API_KEY={}", get("CURSEFORGE_API_KEY"));
    println!("cargo:rerun-if-changed=.env");

    tauri_build::build()
}
