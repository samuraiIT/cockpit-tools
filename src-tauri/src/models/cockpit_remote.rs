use serde::{Deserialize, Serialize};

/// Профиль удалённого хоста для синхронизации Cockpit Tools через SSH.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteHostProfile {
    pub id: String,
    pub name: String,
    pub host: String,
    #[serde(default)]
    pub user: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    #[serde(default)]
    pub identity_file: Option<String>,
    #[serde(default)]
    pub ssh_alias: Option<String>,
    #[serde(default)]
    pub remote_codex_home: Option<String>,
    #[serde(default = "default_local_tunnel_port")]
    pub local_tunnel_port: u16,
    #[serde(default = "default_remote_tunnel_port")]
    pub remote_tunnel_port: u16,
    #[serde(default = "default_true")]
    pub auto_tunnel: bool,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub last_synced_at: Option<i64>,
    #[serde(default)]
    pub last_tunnel_pid: Option<u32>,
    #[serde(default)]
    pub bound_account_id: Option<String>,
    #[serde(default)]
    pub bound_account_email: Option<String>,
}

fn default_ssh_port() -> u16 {
    22
}

fn default_local_tunnel_port() -> u16 {
    19528
}

fn default_remote_tunnel_port() -> u16 {
    19529
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemoteHostStore {
    #[serde(default)]
    pub hosts: Vec<RemoteHostProfile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSshTestResult {
    pub ok: bool,
    pub message: String,
    pub remote_user: Option<String>,
    pub remote_uname: Option<String>,
    pub codex_present: bool,
    pub cockpit_dir_present: bool,
    pub codex_binary_path: Option<String>,
    pub remote_tunnel_listening: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSyncSummary {
    pub host_id: String,
    pub steps: Vec<RemoteSyncStep>,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSyncStep {
    pub name: String,
    pub ok: bool,
    pub detail: Option<String>,
}
