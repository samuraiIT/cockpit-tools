use crate::models::{DefaultInstanceSettings, InstanceProfileView};
use crate::modules;

const DEFAULT_INSTANCE_ID: &str = "__default__";
const CODEX_MULTI_INSTANCE_UNSUPPORTED_REASON: &str =
    "Codex 多开实例暂不支持：官方桌面包主进程存在单实例锁限制。当前仅支持单实例启动、关闭与重启（先关闭再打开）。";

fn codex_multi_instance_unsupported() -> String {
    CODEX_MULTI_INSTANCE_UNSUPPORTED_REASON.to_string()
}

fn resolve_default_account_id(settings: &DefaultInstanceSettings) -> Option<String> {
    if settings.follow_local_account {
        resolve_local_account_id()
    } else {
        settings.bind_account_id.clone()
    }
}

fn resolve_local_account_id() -> Option<String> {
    let account = modules::codex_account::get_current_account()?;
    Some(account.id)
}

#[tauri::command]
pub async fn codex_get_instance_defaults() -> Result<modules::instance::InstanceDefaults, String> {
    modules::codex_instance::get_instance_defaults()
}

#[tauri::command]
pub async fn codex_list_instances() -> Result<Vec<InstanceProfileView>, String> {
    let default_dir = modules::codex_instance::get_default_codex_home()?;
    let default_dir_str = default_dir.to_string_lossy().to_string();
    let default_settings = modules::codex_instance::load_default_settings()?;
    let default_pid = modules::process::resolve_codex_pid(default_settings.last_pid, None);
    let default_running = default_pid.is_some();
    let default_bind_account_id = resolve_default_account_id(&default_settings);

    Ok(vec![InstanceProfileView {
        id: DEFAULT_INSTANCE_ID.to_string(),
        name: String::new(),
        user_data_dir: default_dir_str,
        extra_args: default_settings.extra_args,
        bind_account_id: default_bind_account_id,
        created_at: 0,
        last_launched_at: None,
        last_pid: default_pid,
        running: default_running,
        initialized: modules::instance::is_profile_initialized(&default_dir),
        is_default: true,
        follow_local_account: default_settings.follow_local_account,
    }])
}

#[tauri::command]
pub async fn codex_create_instance(
    _name: String,
    _user_data_dir: String,
    _extra_args: Option<String>,
    _bind_account_id: Option<String>,
    _copy_source_instance_id: Option<String>,
    _init_mode: Option<String>,
) -> Result<InstanceProfileView, String> {
    Err(codex_multi_instance_unsupported())
}

#[tauri::command]
pub async fn codex_update_instance(
    instance_id: String,
    _name: Option<String>,
    extra_args: Option<String>,
    bind_account_id: Option<Option<String>>,
    follow_local_account: Option<bool>,
) -> Result<InstanceProfileView, String> {
    if instance_id != DEFAULT_INSTANCE_ID {
        return Err(codex_multi_instance_unsupported());
    }

    let default_dir = modules::codex_instance::get_default_codex_home()?;
    let default_dir_str = default_dir.to_string_lossy().to_string();
    let updated = modules::codex_instance::update_default_settings(
        bind_account_id,
        extra_args,
        follow_local_account,
    )?;
    let resolved_pid = modules::process::resolve_codex_pid(updated.last_pid, None);
    let running = resolved_pid.is_some();
    let default_bind_account_id = resolve_default_account_id(&updated);
    Ok(InstanceProfileView {
        id: DEFAULT_INSTANCE_ID.to_string(),
        name: String::new(),
        user_data_dir: default_dir_str,
        extra_args: updated.extra_args,
        bind_account_id: default_bind_account_id,
        created_at: 0,
        last_launched_at: None,
        last_pid: resolved_pid,
        running,
        initialized: modules::instance::is_profile_initialized(&default_dir),
        is_default: true,
        follow_local_account: updated.follow_local_account,
    })
}

#[tauri::command]
pub async fn codex_delete_instance(instance_id: String) -> Result<(), String> {
    if instance_id == DEFAULT_INSTANCE_ID {
        return Err("默认实例不可删除".to_string());
    }
    Err(codex_multi_instance_unsupported())
}

#[tauri::command]
pub async fn codex_start_instance(instance_id: String) -> Result<InstanceProfileView, String> {
    modules::process::ensure_codex_launch_path_configured()?;

    if instance_id != DEFAULT_INSTANCE_ID {
        return Err(codex_multi_instance_unsupported());
    }

    let default_dir = modules::codex_instance::get_default_codex_home()?;
    let default_dir_str = default_dir.to_string_lossy().to_string();
    let default_settings = modules::codex_instance::load_default_settings()?;
    let default_bind_account_id = resolve_default_account_id(&default_settings);

    // 单实例重启链路：先关闭，再打开。
    modules::process::close_codex(20)?;
    let _ = modules::codex_instance::clear_all_pids();

    if let Some(ref account_id) = default_bind_account_id {
        modules::codex_instance::inject_account_to_profile(&default_dir, account_id).await?;
    }

    let pid = modules::process::start_codex_default()?;
    let _ = modules::codex_instance::update_default_pid(Some(pid))?;
    let running = modules::process::is_pid_running(pid);
    Ok(InstanceProfileView {
        id: DEFAULT_INSTANCE_ID.to_string(),
        name: String::new(),
        user_data_dir: default_dir_str,
        extra_args: default_settings.extra_args,
        bind_account_id: default_bind_account_id,
        created_at: 0,
        last_launched_at: None,
        last_pid: Some(pid),
        running,
        initialized: modules::instance::is_profile_initialized(&default_dir),
        is_default: true,
        follow_local_account: default_settings.follow_local_account,
    })
}

#[tauri::command]
pub async fn codex_stop_instance(instance_id: String) -> Result<InstanceProfileView, String> {
    if instance_id != DEFAULT_INSTANCE_ID {
        return Err(codex_multi_instance_unsupported());
    }

    let default_dir = modules::codex_instance::get_default_codex_home()?;
    let default_dir_str = default_dir.to_string_lossy().to_string();
    modules::process::close_codex(20)?;
    let _ = modules::codex_instance::clear_all_pids();
    let default_settings = modules::codex_instance::load_default_settings()?;
    let default_bind_account_id = resolve_default_account_id(&default_settings);
    Ok(InstanceProfileView {
        id: DEFAULT_INSTANCE_ID.to_string(),
        name: String::new(),
        user_data_dir: default_dir_str,
        extra_args: default_settings.extra_args,
        bind_account_id: default_bind_account_id,
        created_at: 0,
        last_launched_at: None,
        last_pid: None,
        running: false,
        initialized: modules::instance::is_profile_initialized(&default_dir),
        is_default: true,
        follow_local_account: default_settings.follow_local_account,
    })
}

#[tauri::command]
pub async fn codex_close_all_instances() -> Result<(), String> {
    modules::process::close_codex(20)?;
    let _ = modules::codex_instance::clear_all_pids();
    Ok(())
}

#[tauri::command]
pub async fn codex_open_instance_window(instance_id: String) -> Result<(), String> {
    if instance_id != DEFAULT_INSTANCE_ID {
        return Err(codex_multi_instance_unsupported());
    }

    let default_settings = modules::codex_instance::load_default_settings()?;
    modules::process::focus_codex_instance(default_settings.last_pid, None)
        .map_err(|err| format!("定位 Codex 默认实例窗口失败: {}", err))?;
    Ok(())
}
