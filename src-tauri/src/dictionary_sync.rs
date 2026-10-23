//! Native-only GitHub sync orchestration. No access tokens cross Tauri IPC.
use handy_dictionary_sync::{
    credentials::Vault,
    github::{validate_client_id, DeviceSession, Github},
    plan::Preview,
    storage::{self, SyncState},
    Choice, Dictionary, Rule,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};

#[derive(Default)]
pub struct SyncService(pub tokio::sync::Mutex<Runtime>);

#[derive(Default)]
pub struct Runtime {
    session: Option<DeviceSession>,
    pending: Option<Preview>,
}

fn directory(app: &AppHandle) -> Result<PathBuf, String> {
    crate::portable::app_data_dir(app)
        .map(|p| p.join("dictionary-sync"))
        .map_err(|_| "local_storage".into())
}

fn load(app: &AppHandle) -> Result<SyncState, String> {
    let mut state = storage::load(&directory(app)?)?;
    if state.client_id.is_empty() {
        state.client_id = option_env!("HANDY_GITHUB_CLIENT_ID")
            .unwrap_or_default()
            .to_string();
        state.app_slug = option_env!("HANDY_GITHUB_APP_SLUG")
            .unwrap_or_default()
            .to_string();
    }
    Ok(state)
}

fn snapshot(app: &AppHandle) -> Dictionary {
    let settings = crate::settings::get_settings(app);
    Dictionary {
        words: settings.custom_words,
        replacements: settings
            .dictionary_replacements
            .into_iter()
            .map(|r| Rule {
                from: r.from,
                to: r.to,
            })
            .collect(),
    }
}

fn token(state: &SyncState) -> Result<String, String> {
    Vault::new(&state.client_id)?
        .load()?
        .ok_or_else(|| "not_connected".into())
}

#[tauri::command]
#[specta::specta]
pub async fn dictionary_sync_status(
    app: AppHandle,
    service: State<'_, SyncService>,
) -> Result<Value, String> {
    let _runtime = service.0.try_lock().map_err(|_| "busy")?;
    let state = load(&app)?;
    let connected = if state.client_id.is_empty() {
        false
    } else {
        Vault::new(&state.client_id)?.load()?.is_some()
    };
    Ok(
        json!({ "clientId": state.client_id, "appSlug": state.app_slug, "connected": connected, "repository": state.repository, "lastSync": state.last_sync, "firstSync": state.baseline.is_none() }),
    )
}

#[tauri::command]
#[specta::specta]
pub async fn dictionary_sync_begin(
    app: AppHandle,
    service: State<'_, SyncService>,
    client_id: String,
    app_slug: String,
) -> Result<Value, String> {
    let mut runtime = service.0.try_lock().map_err(|_| "busy")?;
    validate_client_id(&client_id)?;
    handy_dictionary_sync::github::validate_name(&app_slug)?;
    let mut state = load(&app)?;
    // Changing application identity requires explicit Disconnect first.
    if !state.client_id.is_empty() && state.client_id != client_id {
        if Vault::new(&state.client_id)?.load()?.is_some() {
            return Err("disconnect_first".into());
        }
        storage::backup(&directory(&app)?, &snapshot(&app))?;
        state.repository = None;
        state.baseline = None;
        state.last_sync = None;
    }
    let session = Github::new()?.begin(&client_id).await?;
    state.client_id = client_id;
    state.app_slug = app_slug;
    storage::save(&directory(&app)?, &state)?;
    let result = json!({ "userCode": session.user_code, "verificationUri": "https://github.com/login/device", "interval": session.interval });
    runtime.session = Some(session);
    runtime.pending = None;
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn dictionary_sync_poll(
    app: AppHandle,
    service: State<'_, SyncService>,
) -> Result<bool, String> {
    let mut runtime = service.0.try_lock().map_err(|_| "busy")?;
    let state = load(&app)?;
    let session = runtime.session.as_mut().ok_or("authorization_expired")?;
    match Github::new()?.poll(&state.client_id, session).await {
        Ok(Some(token)) => {
            // Never persist the token in state.json or return it to JavaScript.
            let saved = Vault::new(&state.client_id)?.save(&token);
            runtime.session = None;
            saved?;
            Ok(true)
        }
        Ok(None) => Ok(false),
        Err(error) => {
            runtime.session = None;
            Err(error)
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn dictionary_sync_select_repository(
    app: AppHandle,
    service: State<'_, SyncService>,
    owner: String,
    name: String,
) -> Result<Value, String> {
    let mut runtime = service.0.try_lock().map_err(|_| "busy")?;
    let mut state = load(&app)?;
    let repository = Github::new()?
        .repository(&token(&state)?, &owner, &name)
        .await?;
    // Do not discard a valid deletion baseline when the same repo is re-selected.
    if state.repository.as_ref() != Some(&repository) {
        storage::backup(&directory(&app)?, &snapshot(&app))?;
        state.baseline = None;
        state.last_sync = None;
    }
    state.repository = Some(repository.clone());
    storage::save(&directory(&app)?, &state)?;
    runtime.pending = None;
    Ok(json!(repository))
}

#[tauri::command]
#[specta::specta]
pub async fn dictionary_sync_now(
    app: AppHandle,
    service: State<'_, SyncService>,
    resolution: Option<String>,
    confirm_first_sync: bool,
) -> Result<Value, String> {
    let mut runtime = service.0.try_lock().map_err(|_| "busy")?;
    let mut state = load(&app)?;
    let repo = state.repository.as_ref().ok_or("repository_required")?;
    if state.baseline.is_none() && !confirm_first_sync {
        return Err("first_sync_confirmation".into());
    }
    let token = token(&state)?;
    let github = Github::new()?;
    let current_repo = github.repository(&token, &repo.owner, &repo.name).await?;
    if current_repo.id != repo.id {
        return Err("repository_changed".into());
    }
    let local = snapshot(&app);
    handy_dictionary_sync::validate(&local)?;
    let fetched = github.read(&token, repo).await?;
    if fetched.is_none() && state.baseline.is_some() {
        return Err("remote_file_missing".into());
    }
    let (remote, sha) = match fetched {
        Some((dictionary, sha)) => (dictionary, Some(sha)),
        None => (Dictionary::default(), None),
    };
    let choice = match resolution.as_deref() {
        None => None,
        Some("local") | Some("remote") => {
            let previous = runtime.pending.take().ok_or("preview_expired")?;
            previous.check(&local, &remote, &sha)?;
            Some(if resolution.as_deref() == Some("local") {
                Choice::Local
            } else {
                Choice::Remote
            })
        }
        Some(_) => return Err("invalid_resolution".into()),
    };
    let baseline = state.baseline.clone().unwrap_or_default();
    let merged = match handy_dictionary_sync::reconcile(&baseline, &local, &remote, choice) {
        Ok(merged) => merged,
        Err(keys) => {
            let conflicts: Vec<Value> = keys
                .iter()
                .map(|key| {
                    let value = |d: &Dictionary| -> Option<String> {
                        if let Some(word) = key.strip_prefix("word:") {
                            d.words.iter().find(|w| w.to_lowercase() == word).cloned()
                        } else {
                            d.replacements
                                .iter()
                                .find(|r| format!("rule:{}", r.from.to_lowercase()) == *key)
                                .map(|r| format!("{} → {}", r.from, r.to))
                        }
                    };
                    json!({ "key": key, "local": value(&local), "remote": value(&remote) })
                })
                .collect();
            runtime.pending = Some(Preview { local, remote, sha });
            return Ok(json!({ "status": "conflicts", "conflicts": conflicts }));
        }
    };
    handy_dictionary_sync::validate(&merged)?;
    // A backup must succeed before either side is modified.
    storage::backup(&directory(&app)?, &local)?;
    if snapshot(&app) != local {
        return Err("local_changed".into());
    }
    if merged != remote || sha.is_none() {
        github.write(&token, repo, &merged, sha.as_deref()).await?;
    }
    // CAS the local dictionary after network I/O. Concurrent edits are never overwritten.
    crate::settings::update_dictionary(
        &app,
        Some(&local),
        Some(merged.words.clone()),
        Some(
            merged
                .replacements
                .iter()
                .map(
                    |r| crate::audio_toolkit::dictionary::DictionaryReplacement {
                        from: r.from.clone(),
                        to: r.to.clone(),
                    },
                )
                .collect(),
        ),
    )?;
    state.baseline = Some(merged);
    state.last_sync = Some(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "local_storage")?
            .as_secs(),
    );
    storage::save(&directory(&app)?, &state)?;
    runtime.pending = None;
    let _ = app.emit("settings-changed", json!({ "setting": "dictionary" }));
    Ok(json!({ "status": "synced", "lastSync": state.last_sync }))
}

#[tauri::command]
#[specta::specta]
pub async fn dictionary_sync_disconnect(
    app: AppHandle,
    service: State<'_, SyncService>,
) -> Result<(), String> {
    let mut runtime = service.0.try_lock().map_err(|_| "busy")?;
    let state = load(&app)?;
    runtime.session = None;
    runtime.pending = None;
    if !state.client_id.is_empty() {
        Vault::new(&state.client_id)?.delete()?;
    }
    // Keep repo and baseline so reconnecting does not resurrect deleted entries.
    Ok(())
}
