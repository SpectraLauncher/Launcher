use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "version", rename_all = "lowercase")]
pub enum Loader {
    Vanilla,
    Fabric(String),
    Quilt(String),
    Forge(String),
    NeoForge(String),
}

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShareOrigin {
    pub code: String,
    pub revision: u32,
    #[serde(default)]
    pub item_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    #[serde(default)]
    pub loader: Loader,
    #[serde(default)]
    pub memory_mb: Option<u32>,
    #[serde(default)]
    pub java_args: Vec<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub last_played: Option<String>,
    #[serde(default)]
    pub playtime_seconds: u64,
    #[serde(default)]
    pub share_origin: Option<ShareOrigin>,

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

    #[serde(default)]
    pub modpack_project_id: Option<String>,
    #[serde(default)]
    pub modpack_version_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    #[default]
    Microsoft,
    Offline,
}

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
    #[serde(default)]
    pub exp: u64,
    #[serde(default)]
    pub client_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountsFile {
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub active_uuid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub default_memory_mb: u32,
    #[serde(default)]
    pub last_instance_id: Option<String>,
    #[serde(default = "default_theme")]
    pub theme: String,

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

    #[serde(default = "default_true")]
    pub snapshot_before_updates: bool,
    #[serde(default = "default_snapshot_keep")]
    pub snapshot_keep: u32,

    #[serde(default = "default_true")]
    pub track_playtime: bool,
    #[serde(default = "default_true")]
    pub discord_rpc: bool,
    #[serde(default = "default_true")]
    pub crash_reports: bool,
    #[serde(default = "default_true")]
    pub anonymous_stats: bool,
    #[serde(default = "default_true")]
    pub share_activity: bool,
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
            discord_rpc: true,
            crash_reports: true,
            anonymous_stats: true,
            share_activity: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSkin {
    pub id: String,
    pub name: String,
    #[serde(default = "default_skin_model")]
    pub model: String,
    #[serde(default)]
    pub active: bool,
    pub created_at: String,
}

fn default_skin_model() -> String {
    "classic".to_string()
}
