//! Serializable data shared between the Rust backend and the Nuxt frontend.
//!
//! These are intentionally launcher-specific and independent from Lyceris' own
//! `Config` — we persist *our* shape (`instance.json`, `accounts.json`,
//! `launcher.json`) and translate to a Lyceris `Config` only at launch time.

use serde::{Deserialize, Serialize};

/// Which mod loader an instance uses. `Vanilla` carries no version.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "version", rename_all = "lowercase")]
pub enum Loader {
    Vanilla,
    Fabric(String),
    Quilt(String),
    Forge(String),
    NeoForge(String),
}

/// A single environment variable for the game process.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

impl Default for Loader {
    fn default() -> Self {
        Loader::Vanilla
    }
}

/// Where a shared instance came from, and what it contained last time it was
/// synced. The id list is what makes "the author removed a mod" different from
/// "the player added one" — only the first kind is taken away again.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShareOrigin {
    pub code: String,
    pub revision: u32,
    #[serde(default)]
    pub item_ids: Vec<String>,
}

/// One Minecraft instance/profile. Persisted as `<instance>/instance.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    /// Vanilla Minecraft version, e.g. "1.21.4".
    pub mc_version: String,
    #[serde(default)]
    pub loader: Loader,
    /// Max heap in MB. `None` => use launcher default.
    #[serde(default)]
    pub memory_mb: Option<u32>,
    /// Extra JVM args appended after the launcher's defaults.
    #[serde(default)]
    pub java_args: Vec<String>,
    /// Optional icon identifier (built-in name or relative path under the instance).
    #[serde(default)]
    pub icon: Option<String>,
    /// Optional grouping label for the instance grid.
    #[serde(default)]
    pub group: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub last_played: Option<String>,
    /// Total accumulated playtime in seconds.
    #[serde(default)]
    pub playtime_seconds: u64,
    /// Set when this instance came from someone else's share code, so a later
    /// revision can be applied on top instead of landing as a second instance.
    #[serde(default)]
    pub share_origin: Option<ShareOrigin>,

    // --- per-instance launch overrides (each `override_*` flag means "use these
    //     instead of the global defaults") ---
    #[serde(default)]
    pub override_memory: bool,
    #[serde(default)]
    pub override_window: bool,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub override_java_args: bool,
    #[serde(default)]
    pub override_java: bool,
    /// Custom Java executable path (persisted; the engine manages Java itself).
    #[serde(default)]
    pub java_path: Option<String>,
    #[serde(default)]
    pub override_env: bool,
    #[serde(default)]
    pub env_vars: Vec<EnvVar>,
    #[serde(default)]
    pub override_hooks: bool,
    #[serde(default)]
    pub pre_launch: Option<String>,
    #[serde(default)]
    pub wrapper: Option<String>,
    #[serde(default)]
    pub post_exit: Option<String>,

    // --- Modrinth modpack identity (set when created from a .mrpack) ---
    #[serde(default)]
    pub modpack_project_id: Option<String>,
    #[serde(default)]
    pub modpack_version_id: Option<String>,
}

/// How an account authenticates. Offline accounts have no tokens and can only
/// join offline-mode servers / singleplayer.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    #[default]
    Microsoft,
    Offline,
}

/// A persisted account. For Microsoft accounts the token fields mirror
/// `lyceris::auth::microsoft::MinecraftAccount`; for offline accounts they're
/// empty and only `uuid` + `username` matter.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Account {
    #[serde(default)]
    pub kind: AccountKind,
    pub uuid: String,
    pub username: String,
    #[serde(default)]
    pub xuid: String,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    /// Unix expiry of the access token (seconds). 0 for offline.
    #[serde(default)]
    pub exp: u64,
    #[serde(default)]
    pub client_id: String,
}

/// `accounts.json` root.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountsFile {
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub active_uuid: Option<String>,
}

/// `launcher.json` — global settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Default max heap (MB) for instances that don't override it.
    pub default_memory_mb: u32,
    /// Last opened instance (for "continue playing").
    #[serde(default)]
    pub last_instance_id: Option<String>,
    #[serde(default = "default_theme")]
    pub theme: String,

    // --- default instance launch options (used unless an instance overrides) ---
    #[serde(default)]
    pub default_fullscreen: bool,
    #[serde(default)]
    pub default_width: Option<u32>,
    #[serde(default)]
    pub default_height: Option<u32>,
    #[serde(default)]
    pub default_java_path: Option<String>,
    #[serde(default)]
    pub default_java_args: Vec<String>,
    #[serde(default)]
    pub default_env_vars: Vec<EnvVar>,
    #[serde(default)]
    pub default_pre_launch: Option<String>,
    #[serde(default)]
    pub default_wrapper: Option<String>,
    #[serde(default)]
    pub default_post_exit: Option<String>,

    /// Take a restore point before anything that rewrites an instance's content
    /// (a pushed share update, a modpack update). On by default: the operations
    /// it guards can delete mods, and nothing else undoes that.
    #[serde(default = "default_true")]
    pub snapshot_before_updates: bool,
    /// How many *automatic* restore points to keep per instance. Ones taken by
    /// hand are never pruned.
    #[serde(default = "default_snapshot_keep")]
    pub snapshot_keep: u32,

    // --- privacy ---
    /// Accumulate per-instance playtime.
    #[serde(default = "default_true")]
    pub track_playtime: bool,
    /// Show the current instance as Discord Rich Presence while playing.
    #[serde(default)]
    pub discord_rpc: bool,
    /// Send anonymous crash reports (opt-in; not wired yet).
    #[serde(default)]
    pub crash_reports: bool,
    /// Send anonymous usage statistics (opt-in; not wired yet).
    #[serde(default)]
    pub anonymous_stats: bool,
}

fn default_true() -> bool {
    true
}

fn default_snapshot_keep() -> u32 {
    5
}

fn default_theme() -> String {
    "dark".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_memory_mb: 4096,
            last_instance_id: None,
            theme: default_theme(),
            default_fullscreen: false,
            default_width: None,
            default_height: None,
            default_java_path: None,
            default_java_args: Vec::new(),
            default_env_vars: Vec::new(),
            default_pre_launch: None,
            default_wrapper: None,
            default_post_exit: None,
            snapshot_before_updates: true,
            snapshot_keep: default_snapshot_keep(),
            track_playtime: true,
            discord_rpc: false,
            crash_reports: false,
            anonymous_stats: true,
        }
    }
}

/// A saved skin entry in `skins/skins.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSkin {
    pub id: String,
    pub name: String,
    /// "classic" or "slim".
    #[serde(default = "default_skin_model")]
    pub model: String,
    /// Whether this is the skin currently applied to the active account.
    #[serde(default)]
    pub active: bool,
    pub created_at: String,
}

fn default_skin_model() -> String {
    "classic".to_string()
}
