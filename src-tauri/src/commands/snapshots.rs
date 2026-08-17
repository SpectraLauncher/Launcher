//! Restore points for an instance.
//!
//! A snapshot is the same `.zip` a share is: the content index (every mod by
//! provider and version id), the config folder, and the bytes of anything that
//! matches no provider. That makes it small — a 150-mod instance is a couple of
//! hundred kilobytes plus whatever local jars are in it — because the mods
//! themselves are a list to re-download, not a copy.
//!
//! What it deliberately does not hold: worlds and screenshots. They are large,
//! nothing here touches them, and a "restore point" that quietly rolled a world
//! back would be worse than none.
//!
//! Restoring is authoritative: the snapshot *is* the wanted state, so content
//! added afterwards is removed. That is the difference from applying a shared
//! pack, which may only take back what it once installed.

use std::collections::HashSet;
use std::io::Read;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::commands::share::{
    self, folder_for_kind, loader_str, plan_sync, ShareManifest, SyncPlan, FORMAT, ICON,
    KEEP_ON_SYNC, MANIFEST,
};
use crate::commands::{curseforge, import, modrinth, settings};
use crate::models::Instance;
use crate::{paths, store};

/// One restore point, as the panel lists it.
#[derive(Serialize, Deserialize, Clone)]
pub struct Snapshot {
    /// File name inside the instance's `snapshots/` folder — also its id.
    pub file: String,
    pub label: String,
    /// Unix ms.
    pub created: i64,
    pub size: u64,
    /// Taken by the launcher before an update, rather than by hand.
    pub auto: bool,
    /// How many projects the instance had at the time.
    pub items: usize,
}

#[derive(Serialize, Deserialize, Default)]
struct SnapshotIndex {
    #[serde(default)]
    snapshots: Vec<Snapshot>,
}

/// What restoring changed.
#[derive(Serialize)]
pub struct RestoreResult {
    pub installed: usize,
    pub removed: usize,
    pub failed: Vec<String>,
    pub needs_curseforge: usize,
}

fn snapshots_dir(id: &str) -> std::path::PathBuf {
    paths::instance_dir(id).join("snapshots")
}

fn index_file(id: &str) -> std::path::PathBuf {
    snapshots_dir(id).join("index.json")
}

fn read_index(id: &str) -> SnapshotIndex {
    store::read_json(&index_file(id)).ok().flatten().unwrap_or_default()
}

fn write_index(id: &str, index: &SnapshotIndex) -> Result<(), String> {
    store::write_json(&index_file(id), index)
}

#[tauri::command]
pub fn list_snapshots(id: String) -> Result<Vec<Snapshot>, String> {
    let mut list = read_index(&id).snapshots;
    // Newest first, and only the ones whose file is still there — a folder can
    // be cleaned out from underneath us.
    let dir = snapshots_dir(&id);
    list.retain(|s| dir.join(&s.file).exists());
    list.sort_by(|a, b| b.created.cmp(&a.created));
    Ok(list)
}

/// Takes a restore point. `auto` marks the ones the launcher took by itself.
#[tauri::command]
pub async fn create_snapshot(
    app: AppHandle,
    id: String,
    label: Option<String>,
    auto: bool,
) -> Result<Snapshot, String> {
    let instance: Instance =
        store::read_json(&paths::instance_config_file(&id))?.ok_or("instance not found")?;

    let items = modrinth::read_content_index(&id).items;
    let (unresolved, _) = share::scan_unresolved(&id, &items);
    // Everything provider-less goes in: a restore point that cannot put back a
    // hand-dropped jar is not one.
    let paths_set: HashSet<String> = unresolved.iter().map(|u| u.path.clone()).collect();

    let manifest = ShareManifest {
        format: FORMAT.into(),
        version: 1,
        name: instance.name.clone(),
        mc_version: instance.mc_version.clone(),
        loader: instance.loader.clone(),
        items: items.clone(),
        unresolved: paths_set.iter().cloned().collect(),
    };

    let created = chrono::Utc::now().timestamp_millis();
    let dir = snapshots_dir(&id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create snapshots dir: {e}"))?;
    let file = format!("{created}.zip");
    let dest = dir.join(&file);

    let _ = app.emit("snapshot://progress", serde_json::json!({ "instance_id": id, "stage": "packing" }));
    share::write_pack(&dest, &manifest, &id, &paths_set, &paths_set, &mut |_| {})?;

    let size = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
    let snapshot = Snapshot {
        file,
        label: label.unwrap_or_default(),
        created,
        size,
        auto,
        items: items.len(),
    };

    let mut index = read_index(&id);
    index.snapshots.push(snapshot.clone());
    prune(&id, &mut index);
    write_index(&id, &index)?;

    let _ = app.emit("snapshot://progress", serde_json::json!({ "instance_id": id, "stage": "done" }));
    Ok(snapshot)
}

/// Deletes the oldest automatic snapshots past the keep limit.
///
/// Only the automatic ones: a restore point somebody took by hand, before doing
/// something they were unsure about, is not the launcher's to throw away.
fn prune(id: &str, index: &mut SnapshotIndex) {
    let keep = settings::get_settings()
        .map(|s| s.snapshot_keep.max(1) as usize)
        .unwrap_or(5);

    let mut by_age: Vec<Snapshot> = index.snapshots.iter().filter(|s| s.auto).cloned().collect();
    by_age.sort_by_key(|s| s.created);
    let over = by_age.len().saturating_sub(keep);
    let dir = snapshots_dir(id);
    for old in by_age.into_iter().take(over) {
        let _ = std::fs::remove_file(dir.join(&old.file));
        index.snapshots.retain(|s| s.file != old.file);
    }
}

#[tauri::command]
pub fn delete_snapshot(id: String, file: String) -> Result<(), String> {
    // The name comes from the UI, so it must not be able to point outside.
    if file.contains('/') || file.contains('\\') || file.contains("..") {
        return Err("bad snapshot name".into());
    }
    let _ = std::fs::remove_file(snapshots_dir(&id).join(&file));
    let mut index = read_index(&id);
    index.snapshots.retain(|s| s.file != file);
    write_index(&id, &index)
}

/// Puts the instance back to the state a snapshot describes.
///
/// Takes a snapshot of the current state first, labelled automatically: undoing
/// a restore is exactly the moment somebody realises they wanted the other one.
#[tauri::command]
pub async fn restore_snapshot(
    app: AppHandle,
    id: String,
    file: String,
) -> Result<RestoreResult, String> {
    if file.contains('/') || file.contains('\\') || file.contains("..") {
        return Err("bad snapshot name".into());
    }
    let path = snapshots_dir(&id).join(&file);
    let bytes = std::fs::read(&path).map_err(|e| format!("read snapshot: {e}"))?;

    let _ = create_snapshot(app.clone(), id.clone(), Some("before restore".into()), true).await;

    let mut instance: Instance =
        store::read_json(&paths::instance_config_file(&id))?.ok_or("instance not found")?;

    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("open snapshot: {e}"))?;
    let manifest: ShareManifest = {
        let mut f = archive.by_name(MANIFEST).map_err(|_| "not a Spectra snapshot".to_string())?;
        let mut text = String::new();
        f.read_to_string(&mut text).map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| format!("parse manifest: {e}"))?
    };

    let installed_now = modrinth::read_content_index(&id).items;
    // Authoritative: everything installed is fair game to remove, because the
    // snapshot is the state being asked for.
    let owned: HashSet<String> = installed_now.iter().map(|i| i.project_id.clone()).collect();
    let SyncPlan { install: todo, remove } = plan_sync(&manifest.items, &installed_now, &owned);

    let game_dir = paths::instance_game_dir(&id);
    let mut removed = 0usize;
    for item in remove {
        let file = game_dir.join(folder_for_kind(&item.kind)).join(&item.filename);
        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_file(file.with_extension("jar.disabled"));
        modrinth::remove_index_entry(&id, &item.filename);
        removed += 1;
    }

    if let Ok(mut entry) = archive.by_name(ICON) {
        let mut buf = Vec::new();
        if entry.read_to_end(&mut buf).is_ok()
            && std::fs::write(paths::instance_icon_file(&id), &buf).is_ok()
        {
            instance.icon = Some(ICON.to_string());
            let _ = store::write_json(&paths::instance_config_file(&id), &instance);
        }
    }

    // Configs and the local jars come back byte for byte; the player's own
    // settings files are left alone, same as when a shared pack updates.
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let Some(rel) = entry.name().strip_prefix("overrides/").map(str::to_string) else { continue };
        if KEEP_ON_SYNC.iter().any(|k| rel.eq_ignore_ascii_case(k)) {
            continue;
        }
        let dest = import::join_safe(&game_dir, &rel)?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        std::fs::write(&dest, &buf).map_err(|e| format!("write {rel}: {e}"))?;
    }

    let cf_enabled = curseforge::cf_enabled();
    let loader = loader_str(&manifest.loader);
    let mc = Some(manifest.mc_version.clone());
    let total = todo.len() as u64;
    let mut installed = 0usize;
    let mut failed: Vec<String> = Vec::new();
    let mut needs_curseforge = 0usize;

    for (i, item) in todo.iter().enumerate() {
        let outcome = if item.provider == "curseforge" {
            if !cf_enabled {
                needs_curseforge += 1;
                continue;
            }
            curseforge::curseforge_install_with_deps(
                id.clone(),
                item.project_id.clone(),
                item.version_id.clone(),
                mc.clone(),
                loader.clone(),
            )
            .await
            .map(|_| ())
        } else {
            modrinth::modrinth_install_with_deps(
                id.clone(),
                item.version_id.clone(),
                mc.clone(),
                loader.clone(),
            )
            .await
            .map(|_| ())
        };

        if let Err(e) = outcome {
            log::warn!("restore: {} failed: {e}", item.name);
            failed.push(item.name.clone());
        } else {
            installed += 1;
        }

        // Reuse the modpack channel so the titlebar shows progress as usual.
        let _ = app.emit(
            "modrinth://modpack-progress",
            serde_json::json!({
                "instance_id": id,
                "name": manifest.name,
                "current": i as u64 + 1,
                "total": total,
            }),
        );
    }

    Ok(RestoreResult { installed, removed, failed, needs_curseforge })
}

/// Takes an automatic snapshot before something that rewrites content, unless
/// the setting is off. Never fails the operation it is protecting — a missing
/// restore point is bad, refusing to update because of it is worse.
pub async fn snapshot_before(app: &AppHandle, id: &str, reason: &str) {
    let enabled = settings::get_settings().map(|s| s.snapshot_before_updates).unwrap_or(true);
    if !enabled {
        return;
    }
    if let Err(e) = create_snapshot(app.clone(), id.to_string(), Some(reason.to_string()), true).await
    {
        log::warn!("could not take a snapshot before {reason}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::{Snapshot, SnapshotIndex};

    fn snap(created: i64, auto: bool) -> Snapshot {
        Snapshot {
            file: format!("{created}.zip"),
            label: String::new(),
            created,
            size: 0,
            auto,
            items: 0,
        }
    }

    /// The same decision `prune` makes, without touching the disk: keep the
    /// newest `keep` automatic points, and never touch a hand-made one.
    fn survivors(index: &SnapshotIndex, keep: usize) -> Vec<i64> {
        let mut autos: Vec<&Snapshot> = index.snapshots.iter().filter(|s| s.auto).collect();
        autos.sort_by_key(|s| s.created);
        let over = autos.len().saturating_sub(keep);
        let doomed: Vec<String> = autos.into_iter().take(over).map(|s| s.file.clone()).collect();
        index
            .snapshots
            .iter()
            .filter(|s| !doomed.contains(&s.file))
            .map(|s| s.created)
            .collect()
    }

    #[test]
    fn pruning_keeps_the_newest_and_spares_manual_ones() {
        let index = SnapshotIndex {
            snapshots: vec![
                snap(1, true),
                snap(2, false), // taken by hand before something risky
                snap(3, true),
                snap(4, true),
            ],
        };
        let left = survivors(&index, 2);
        assert!(left.contains(&2), "a hand-made point must never be pruned");
        assert!(left.contains(&3) && left.contains(&4), "the newest automatic ones stay");
        assert!(!left.contains(&1), "the oldest automatic one goes");
    }

    #[test]
    fn nothing_is_pruned_under_the_limit() {
        let index = SnapshotIndex { snapshots: vec![snap(1, true), snap(2, true)] };
        assert_eq!(survivors(&index, 5).len(), 2);
    }
}
