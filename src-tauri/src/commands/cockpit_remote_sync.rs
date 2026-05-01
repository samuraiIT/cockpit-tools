use crate::models::cockpit_remote::{
    RemoteHostProfile, RemoteSshTestResult, RemoteSyncSummary,
};
use crate::modules::cockpit_remote_sync;

#[tauri::command]
pub async fn cockpit_remote_list_hosts() -> Result<Vec<RemoteHostProfile>, String> {
    cockpit_remote_sync::list_hosts()
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn cockpit_remote_add_host(
    name: String,
    host: String,
    user: String,
    port: Option<u16>,
    identity_file: Option<String>,
    ssh_alias: Option<String>,
    remote_codex_home: Option<String>,
    local_tunnel_port: Option<u16>,
    remote_tunnel_port: Option<u16>,
    auto_tunnel: Option<bool>,
) -> Result<RemoteHostProfile, String> {
    cockpit_remote_sync::add_host(cockpit_remote_sync::CreateRemoteHostParams {
        name,
        host,
        user,
        port,
        identity_file,
        ssh_alias,
        remote_codex_home,
        local_tunnel_port,
        remote_tunnel_port,
        auto_tunnel,
    })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn cockpit_remote_update_host(
    id: String,
    name: Option<String>,
    host: Option<String>,
    user: Option<String>,
    port: Option<u16>,
    identity_file: Option<Option<String>>,
    ssh_alias: Option<Option<String>>,
    remote_codex_home: Option<Option<String>>,
    local_tunnel_port: Option<u16>,
    remote_tunnel_port: Option<u16>,
    auto_tunnel: Option<bool>,
    bound_account_id: Option<Option<String>>,
    bound_account_email: Option<Option<String>>,
) -> Result<RemoteHostProfile, String> {
    cockpit_remote_sync::update_host(cockpit_remote_sync::UpdateRemoteHostParams {
        id,
        name,
        host,
        user,
        port,
        identity_file,
        ssh_alias,
        remote_codex_home,
        local_tunnel_port,
        remote_tunnel_port,
        auto_tunnel,
        bound_account_id,
        bound_account_email,
    })
}

#[tauri::command]
pub async fn cockpit_remote_delete_host(id: String) -> Result<(), String> {
    cockpit_remote_sync::delete_host(&id)
}

#[tauri::command]
pub async fn cockpit_remote_test_connection(id: String) -> Result<RemoteSshTestResult, String> {
    cockpit_remote_sync::test_connection(&id)
}

#[tauri::command]
pub async fn cockpit_remote_ensure_codex_path(id: String) -> Result<String, String> {
    cockpit_remote_sync::ensure_remote_codex_path(&id)
}

#[tauri::command]
pub async fn cockpit_remote_fetch_codex_auth(id: String) -> Result<String, String> {
    cockpit_remote_sync::fetch_remote_codex_auth(&id)
}

#[tauri::command]
pub async fn cockpit_remote_push_codex_auth(
    id: String,
    local_auth_path: String,
) -> Result<String, String> {
    cockpit_remote_sync::push_codex_auth_to_remote(&id, &local_auth_path)
}

#[tauri::command]
pub async fn cockpit_remote_start_tunnel(id: String) -> Result<u32, String> {
    cockpit_remote_sync::start_tunnel(&id)
}

#[tauri::command]
pub async fn cockpit_remote_stop_tunnel(id: String) -> Result<(), String> {
    cockpit_remote_sync::stop_tunnel(&id)
}

#[tauri::command]
pub async fn cockpit_remote_apply_full_setup(
    id: String,
    local_auth_path: Option<String>,
) -> Result<RemoteSyncSummary, String> {
    cockpit_remote_sync::apply_full_setup(&id, local_auth_path)
}
