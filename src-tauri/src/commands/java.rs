use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JavaInstallation {
    pub path: String,
    pub version: Option<String>,
    pub major: Option<u32>,
    pub vendor: Option<String>,
    pub arch: Option<String>,
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaValidation {
    pub is_valid: bool,
    pub version: Option<String>,
    pub major: Option<u32>,
    pub vendor: Option<String>,
    pub arch: Option<String>,
    pub error: Option<String>,
}

struct JavaVersionInfo {
    version: String,
    major: u32,
    vendor: Option<String>,
    arch: Option<String>,
}

#[tauri::command]
pub fn detect_java_installations() -> Vec<JavaInstallation> {
    let mut installations = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for path in collect_java_candidates() {
        let key = path.to_string_lossy().to_string();
        if !seen.insert(key) {
            continue;
        }
        if let Some(inst) = validate_and_create_installation(&path) {
            installations.push(inst);
        }
    }

    installations.sort_by(|a, b| match (b.major, a.major) {
        (Some(b_major), Some(a_major)) => b_major.cmp(&a_major),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.path.cmp(&b.path),
    });
    installations
}

#[tauri::command]
pub fn validate_java_path(path: String) -> JavaValidation {
    let p = Path::new(&path);
    if !p.exists() {
        return JavaValidation {
            is_valid: false,
            version: None,
            major: None,
            vendor: None,
            arch: None,
            error: Some("Path does not exist".into()),
        };
    }
    match get_java_version_info(p) {
        Ok(info) => JavaValidation {
            is_valid: true,
            version: Some(info.version),
            major: Some(info.major),
            vendor: info.vendor,
            arch: info.arch,
            error: None,
        },
        Err(e) => JavaValidation {
            is_valid: false,
            version: None,
            major: None,
            vendor: None,
            arch: None,
            error: Some(e),
        },
    }
}

fn validate_and_create_installation(path: &Path) -> Option<JavaInstallation> {
    get_java_version_info(path).ok().map(|info| JavaInstallation {
        path: path.to_string_lossy().to_string(),
        version: Some(info.version),
        major: Some(info.major),
        vendor: info.vendor,
        arch: info.arch,
        is_valid: true,
    })
}

fn get_java_version_info(java_path: &Path) -> Result<JavaVersionInfo, String> {
    if !java_path.is_file() {
        return Err("Not a file".into());
    }
    let mut command = Command::new(java_path);
    command.arg("-version");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().map_err(|e| format!("run java -version: {e}"))?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    parse_java_version_output(&combined)
}

fn parse_java_version_output(output: &str) -> Result<JavaVersionInfo, String> {
    let first = output.lines().next().unwrap_or("");
    let version = extract_version_string(first).ok_or("could not parse Java version")?;
    Ok(JavaVersionInfo {
        major: parse_major_version(&version),
        vendor: detect_vendor(output),
        arch: detect_architecture(output),
        version,
    })
}

fn extract_version_string(line: &str) -> Option<String> {
    let start = line.find('"')?;
    let end = line[start + 1..].find('"')?;
    Some(line[start + 1..start + 1 + end].to_string())
}

fn parse_major_version(version: &str) -> u32 {
    let parts: Vec<&str> = version.split('.').collect();
    if let Some(first) = parts.first() {
        if let Ok(n) = first.parse::<u32>() {
            if n == 1 && parts.len() > 1 {
                if let Ok(second) = parts[1].parse::<u32>() {
                    return second;
                }
            }
            return n;
        }
    }
    0
}

fn detect_vendor(output: &str) -> Option<String> {
    let l = output.to_lowercase();
    if l.contains("temurin") || l.contains("adoptium") {
        Some("Eclipse Temurin".into())
    } else if l.contains("zulu") {
        Some("Azul Zulu".into())
    } else if l.contains("corretto") {
        Some("Amazon Corretto".into())
    } else if l.contains("graalvm") {
        Some("GraalVM".into())
    } else if l.contains("microsoft") {
        Some("Microsoft".into())
    } else if l.contains("openjdk") {
        Some("OpenJDK".into())
    } else if l.contains("oracle") || l.contains("java(tm)") {
        Some("Oracle".into())
    } else {
        None
    }
}

fn detect_architecture(output: &str) -> Option<String> {
    let l = output.to_lowercase();
    if l.contains("aarch64") || l.contains("arm64") {
        Some("aarch64".into())
    } else if l.contains("x86_64") || l.contains("amd64") {
        Some("x86_64".into())
    } else if l.contains("x86") || l.contains("i386") || l.contains("i686") {
        Some("x86".into())
    } else {
        None
    }
}

fn java_executable_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "java.exe"
    }
    #[cfg(not(target_os = "windows"))]
    {
        "java"
    }
}

fn collect_java_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    let runtimes = paths::runtimes_dir();
    if let Ok(entries) = std::fs::read_dir(&runtimes) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(bin) = find_java_executable(&entry.path()) {
                    candidates.push(bin);
                }
            }
        }
    }

    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        candidates.push(Path::new(&java_home).join("bin").join(java_executable_name()));
    }

    #[cfg(target_os = "windows")]
    collect_windows_candidates(&mut candidates);
    #[cfg(not(target_os = "windows"))]
    collect_unix_candidates(&mut candidates);

    candidates
}

#[cfg(target_os = "windows")]
fn collect_windows_candidates(candidates: &mut Vec<PathBuf>) {
    let program_files = [
        std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into()),
        std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| "C:\\Program Files (x86)".into()),
    ];
    let vendor_dirs = [
        "Java", "Eclipse Adoptium", "AdoptOpenJDK", "Microsoft", "Zulu", "Amazon Corretto", "BellSoft",
    ];
    for pf in &program_files {
        for dir in &vendor_dirs {
            let base = Path::new(pf).join(dir);
            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        let java = entry.path().join("bin").join("java.exe");
                        if java.exists() {
                            candidates.push(java);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn collect_unix_candidates(candidates: &mut Vec<PathBuf>) {
    candidates.push(PathBuf::from("/usr/bin/java"));
    for jvm_dir in ["/usr/lib/jvm", "/usr/lib64/jvm", "/usr/java"] {
        if let Ok(entries) = std::fs::read_dir(jvm_dir) {
            for entry in entries.flatten() {
                candidates.push(entry.path().join("bin").join("java"));
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir("/Library/Java/JavaVirtualMachines") {
        for entry in entries.flatten() {
            candidates.push(entry.path().join("Contents/Home/bin/java"));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let sdkman = Path::new(&home).join(".sdkman/candidates/java");
        if let Ok(entries) = std::fs::read_dir(&sdkman) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    candidates.push(entry.path().join("bin").join("java"));
                }
            }
        }
    }
}

fn find_java_executable(dir: &Path) -> Option<PathBuf> {
    let direct = dir.join("bin").join(java_executable_name());
    if direct.exists() {
        return Some(direct);
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let deep = entry.path().join("bin").join(java_executable_name());
                if deep.exists() {
                    return Some(deep);
                }
            }
        }
    }
    None
}
