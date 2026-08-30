use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::Path;

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("parse {}: {e}", path.display()))?;
    Ok(Some(value))
}

/// Reads a file written by [`write_json_private`].
///
/// On Windows those files are DPAPI-encrypted, so they have to come back through
/// the same door. Files written by an older build are still plaintext JSON; they
/// are read as-is and re-sealed, so an existing install upgrades itself the first
/// time it starts.
pub fn read_json_private<T: DeserializeOwned + Serialize>(path: &Path) -> Result<Option<T>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;

    if looks_like_json(&bytes) {
        let value: T = serde_json::from_slice(&bytes)
            .map_err(|e| format!("parse {}: {e}", path.display()))?;
        #[cfg(windows)]
        if let Err(e) = write_json_private(path, &value) {
            log::warn!("could not re-seal {}: {e}", path.display());
        }
        return Ok(Some(value));
    }

    let plain = unseal(&bytes)
        .map_err(|e| format!("decrypt {}: {e}", path.display()))?;
    let value = serde_json::from_slice(&plain)
        .map_err(|e| format!("parse {}: {e}", path.display()))?;
    Ok(Some(value))
}

fn looks_like_json(bytes: &[u8]) -> bool {
    bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{')
}

#[cfg(unix)]
fn restrict(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict(_path: &Path) {}

#[cfg(windows)]
mod dpapi {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
    };

    fn blob(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB { cbData: bytes.len() as u32, pbData: bytes.as_ptr() as *mut u8 }
    }

    // SAFETY: `out` is filled by the call above; Windows allocated pbData with
    // LocalAlloc, so it is copied out and freed here.
    unsafe fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        let copied = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut _)));
        copied
    }

    pub fn seal(plain: &[u8]) -> Result<Vec<u8>, String> {
        let input = blob(plain);
        let mut out = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptProtectData(&input, PCWSTR::null(), None, None, None, 0, &mut out)
                .map_err(|e| format!("CryptProtectData: {e}"))?;
            Ok(take(out))
        }
    }

    pub fn unseal(sealed: &[u8]) -> Result<Vec<u8>, String> {
        let input = blob(sealed);
        let mut out = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptUnprotectData(&input, None, None, None, None, 0, &mut out)
                .map_err(|e| format!("CryptUnprotectData: {e}"))?;
            Ok(take(out))
        }
    }
}

#[cfg(windows)]
fn seal(plain: Vec<u8>) -> Vec<u8> {
    match dpapi::seal(&plain) {
        Ok(sealed) => sealed,
        // Losing the account file is worse than storing it the way every older
        // build already did. Keep the data, say so loudly.
        Err(e) => {
            log::error!("DPAPI could not encrypt the token file, storing it in plaintext: {e}");
            plain
        }
    }
}

#[cfg(not(windows))]
fn seal(plain: Vec<u8>) -> Vec<u8> {
    plain
}

#[cfg(windows)]
fn unseal(sealed: &[u8]) -> Result<Vec<u8>, String> {
    dpapi::unseal(sealed)
}

#[cfg(not(windows))]
fn unseal(_sealed: &[u8]) -> Result<Vec<u8>, String> {
    Err("file is not JSON and this platform does not encrypt token files".into())
}

/// Writes a file that holds credentials: 0600 on Unix, DPAPI-encrypted on
/// Windows, which has no equivalent of the mode bits.
pub fn write_json_private<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(value).map_err(|e| format!("serialize: {e}"))?;
    write_bytes(path, seal(json))
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(value).map_err(|e| format!("serialize: {e}"))?;
    write_bytes(path, json)
}

fn write_bytes(path: &Path, bytes: Vec<u8>) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = parent.join(format!(".{file_name}.{stamp}.{:x}.tmp", std::process::id()));

    std::fs::write(&tmp, &bytes).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    restrict(&tmp);
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("rename {} -> {}: {e}", tmp.display(), path.display()));
    }
    restrict(path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::looks_like_json;

    #[test]
    fn plaintext_and_sealed_files_are_told_apart() {
        assert!(looks_like_json(b"{\"token\":\"x\"}"));
        assert!(looks_like_json(b"\n  {\n  \"accounts\": []\n}"));

        // DPAPI output starts with its own header, never with `{`.
        assert!(!looks_like_json(&[0x01, 0x00, 0x00, 0x00, 0xd0, 0x8c, 0x9d, 0xdf]));
        assert!(!looks_like_json(b""));
        assert!(!looks_like_json(b"[1,2,3]"));
    }
}
