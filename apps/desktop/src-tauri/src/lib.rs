//! Tauri commands: thin wrappers over `uber_skill_core`, with a shared config state.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::State;
use uber_skill_core::{git, install, lint, search, Config, FileDiff, InstalledSkill, Issue, ItemKind, Library, LockEntry, Query, Skill, Target};

struct AppState {
    config: Mutex<Config>,
}

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn open_library(state: &AppState, kind: ItemKind) -> CmdResult<Library> {
    let cfg = state.config.lock().map_err(err)?;
    let path = cfg.path_for(kind).map_err(err)?;
    if kind == ItemKind::Agent && !path.is_dir() {
        std::fs::create_dir_all(&path).map_err(err)?;
    }
    Library::open_kind(path, kind).map_err(err)
}

#[derive(Serialize)]
struct LibraryView {
    kind: ItemKind,
    root: PathBuf,
    skills: Vec<Skill>,
    warnings: Vec<uber_skill_core::ScanWarning>,
    tags: Vec<String>,
    categories: Vec<String>,
}

#[tauri::command]
fn get_config(state: State<AppState>) -> CmdResult<Config> {
    Ok(state.config.lock().map_err(err)?.clone())
}

#[tauri::command]
fn set_library(state: State<AppState>, path: PathBuf) -> CmdResult<Config> {
    let mut cfg = state.config.lock().map_err(err)?;
    let next = git::library_config(&cfg, &path).map_err(err)?;
    next.save().map_err(err)?;
    *cfg = next;
    Ok(cfg.clone())
}

#[tauri::command]
async fn clone_library(state: State<'_, AppState>, url: String, parent: PathBuf, name: String) -> CmdResult<Config> {
    // Network and checkout work run off the UI thread, without locking config.
    let path = tauri::async_runtime::spawn_blocking(move || git::clone_repository(&url, &parent, &name))
        .await.map_err(err)?.map_err(err)?;
    let mut cfg = state.config.lock().map_err(err)?;
    let next = git::library_config(&cfg, &path).map_err(err)?;
    next.save().map_err(|e| format!("Dépôt cloné dans {}, mais configuration non enregistrée : {e}. Vous pouvez ouvrir ce dépôt localement.", path.display()))?;
    *cfg = next;
    Ok(cfg.clone())
}

#[tauri::command]
async fn git_publication_preview(state: State<'_, AppState>) -> CmdResult<git::publication::Preview> {
    let path = state.config.lock().map_err(err)?.library_path().map_err(err)?;
    tauri::async_runtime::spawn_blocking(move || git::publication::preview(&path))
        .await.map_err(err)?.map_err(err)
}

#[tauri::command]
async fn publish_library(state: State<'_, AppState>, snapshot: String, paths: Vec<String>, message: String) -> CmdResult<git::publication::PublicationResult> {
    let path = state.config.lock().map_err(err)?.library_path().map_err(err)?;
    tauri::async_runtime::spawn_blocking(move || git::publication::publish(&path, &snapshot, &paths, &message))
        .await.map_err(err)?.map_err(err)
}

#[tauri::command]
async fn check_library_git(state: State<'_, AppState>) -> CmdResult<git::sync::SyncStatus> {
    let path = state.config.lock().map_err(err)?.library_path().map_err(err)?;
    tauri::async_runtime::spawn_blocking(move || git::sync::check(&path)).await.map_err(err)?.map_err(err)
}

#[tauri::command]
async fn update_library_git(state: State<'_, AppState>, snapshot: String) -> CmdResult<git::sync::SyncStatus> {
    let path = state.config.lock().map_err(err)?.library_path().map_err(err)?;
    tauri::async_runtime::spawn_blocking(move || git::sync::update(&path, &snapshot)).await.map_err(err)?.map_err(err)
}

#[tauri::command]
async fn prepare_install(state: State<'_, AppState>, kind: ItemKind, ids: Vec<String>, target: Target) -> CmdResult<git::installation::InstallationPlan> {
    let cfg = state.config.lock().map_err(err)?.clone();
    tauri::async_runtime::spawn_blocking(move || git::installation::prepare(&cfg, kind, &ids, &target)).await.map_err(err)?.map_err(err)
}

#[tauri::command]
fn set_agents_library(state: State<AppState>, path: Option<PathBuf>) -> CmdResult<Config> {
    if let Some(p) = &path {
        Library::open_kind(p, ItemKind::Agent).map_err(err)?;
    }
    let mut cfg = state.config.lock().map_err(err)?;
    cfg.agents_path = path.map(|p| p.canonicalize().unwrap_or(p));
    cfg.save().map_err(err)?;
    Ok(cfg.clone())
}

#[tauri::command]
fn set_editor(state: State<AppState>, command: Option<String>) -> CmdResult<Config> {
    let mut cfg = state.config.lock().map_err(err)?;
    cfg.editor_command = command.filter(|c| !c.trim().is_empty());
    cfg.save().map_err(err)?;
    Ok(cfg.clone())
}

#[tauri::command]
fn remember_project(state: State<AppState>, path: PathBuf, target: Target) -> CmdResult<Config> {
    let mut cfg = state.config.lock().map_err(err)?;
    cfg.remember_project(path, target);
    cfg.save().map_err(err)?;
    Ok(cfg.clone())
}

#[tauri::command]
fn scan_library(state: State<AppState>, kind: ItemKind) -> CmdResult<LibraryView> {
    let lib = open_library(&state, kind)?;
    let scan = lib.scan().map_err(err)?;
    let (tags, categories) = Library::facets(&scan.skills);
    Ok(LibraryView { kind, root: lib.root().to_path_buf(), skills: scan.skills, warnings: scan.warnings, tags, categories })
}

#[tauri::command]
fn search_skills(state: State<AppState>, kind: ItemKind, query: Query) -> CmdResult<Vec<Skill>> {
    let lib = open_library(&state, kind)?;
    let scan = lib.scan().map_err(err)?;
    Ok(search::search(&scan.skills, &query).into_iter().cloned().collect())
}

#[tauri::command]
fn get_skill(state: State<AppState>, kind: ItemKind, id: String) -> CmdResult<Skill> {
    open_library(&state, kind)?.get(&id).map_err(err)
}

#[tauri::command]
fn read_skill_file(state: State<AppState>, kind: ItemKind, id: String, rel: String) -> CmdResult<String> {
    open_library(&state, kind)?.read_file(&id, &rel).map_err(err)
}

#[tauri::command]
fn write_skill_file(state: State<AppState>, kind: ItemKind, id: String, rel: String, text: String) -> CmdResult<Skill> {
    let lib = open_library(&state, kind)?;
    lib.write_file(&id, &rel, &text).map_err(err)?;
    lib.get(&id).map_err(err)
}

#[derive(Deserialize)]
struct MetaPatch {
    tags: Option<Vec<String>>,
    /// `Some(None)` clears the category; encoded from JS as `{ "category": null }` with `set_category: true`.
    category: Option<String>,
    set_category: bool,
    hosts: Option<Vec<String>>,
    description: Option<String>,
}

#[tauri::command]
fn update_meta(state: State<AppState>, kind: ItemKind, id: String, patch: MetaPatch) -> CmdResult<Skill> {
    let lib = open_library(&state, kind)?;
    let category = if patch.set_category { Some(patch.category.as_deref()) } else { None };
    lib.update_meta(&id, patch.tags.as_deref(), category, patch.hosts.as_deref(), patch.description.as_deref()).map_err(err)
}

#[tauri::command]
fn create_skill(state: State<AppState>, kind: ItemKind, id: String, description: String, category: Option<String>, tags: Vec<String>, hosts: Vec<String>) -> CmdResult<Skill> {
    open_library(&state, kind)?.create(&id, &description, category.as_deref(), &tags, &hosts).map_err(err)
}

/// Warnings for items whose declared hosts exclude `target` (empty = all fine).
#[tauri::command]
fn check_hosts(state: State<AppState>, kind: ItemKind, ids: Vec<String>, target: Target) -> CmdResult<Vec<String>> {
    let lib = open_library(&state, kind)?;
    let mut out = Vec::new();
    if !target.supports(kind) {
        return Err(format!("{} ne gère pas les {}s", target.label(), kind.label()));
    }
    for id in ids {
        let skill = lib.get(&id).map_err(err)?;
        if let Some(w) = install::host_mismatch(&skill, &target) {
            out.push(w);
        }
    }
    Ok(out)
}

#[tauri::command]
fn delete_skill(state: State<AppState>, kind: ItemKind, id: String) -> CmdResult<()> {
    open_library(&state, kind)?.delete(&id).map_err(err)
}

#[tauri::command]
fn import_skill(state: State<AppState>, kind: ItemKind, path: PathBuf, new_id: Option<String>) -> CmdResult<Skill> {
    open_library(&state, kind)?.import(&path, new_id.as_deref()).map_err(err)
}

#[tauri::command]
fn lint_skill(state: State<AppState>, kind: ItemKind, id: String) -> CmdResult<Vec<Issue>> {
    let lib = open_library(&state, kind)?;
    let skill = lib.get(&id).map_err(err)?;
    lint::lint_item(&skill.path, skill.kind).map_err(err)
}

#[tauri::command]
fn project_status(state: State<AppState>, kind: ItemKind, project: PathBuf, target: Target) -> CmdResult<Vec<InstalledSkill>> {
    let lib = open_library(&state, kind).ok();
    install::status(lib.as_ref(), &project, &target, kind).map_err(err)
}

#[tauri::command]
async fn install_skills(state: State<'_, AppState>, plan: git::installation::InstallationPlan, project: PathBuf) -> CmdResult<Vec<LockEntry>> {
    let cfg = state.config.lock().map_err(err)?.clone();
    tauri::async_runtime::spawn_blocking(move || git::installation::install_prepared(&cfg, &plan, &project)).await.map_err(err)?.map_err(err)
}

#[tauri::command]
fn uninstall_skill(kind: ItemKind, id: String, project: PathBuf, target: Target) -> CmdResult<()> {
    install::uninstall(&id, &project, &target, kind).map_err(err)
}

#[tauri::command]
fn diff_installed(state: State<AppState>, kind: ItemKind, id: String, project: PathBuf, target: Target) -> CmdResult<Vec<FileDiff>> {
    let lib = open_library(&state, kind)?;
    install::diff_installed(&lib, &id, &project, &target).map_err(err)
}

#[tauri::command]
fn sync_skill(state: State<AppState>, kind: ItemKind, id: String, direction: String, project: PathBuf, target: Target) -> CmdResult<()> {
    let lib = open_library(&state, kind)?;
    match direction.as_str() {
        "pull" => install::sync_to_project(&lib, &id, &project, &target).map(|_| ()).map_err(err),
        "push" => install::sync_to_library(&lib, &id, &project, &target).map(|_| ()).map_err(err),
        other => Err(format!("unknown direction {other}")),
    }
}

#[tauri::command]
fn adopt_skill(state: State<AppState>, kind: ItemKind, id: String, project: PathBuf, target: Target) -> CmdResult<LockEntry> {
    let lib = open_library(&state, kind)?;
    install::adopt(&lib, &id, &project, &target).map_err(err)
}

/// Open a path in the configured editor, falling back to `code`, then the OS default.
#[tauri::command]
fn open_in_editor(state: State<AppState>, path: PathBuf) -> CmdResult<()> {
    let editor = state.config.lock().map_err(err)?.editor_command.clone();
    let candidates: Vec<Vec<String>> = match editor {
        Some(cmd) => {
            let mut parts: Vec<String> = cmd.split_whitespace().map(String::from).collect();
            parts.push(path.display().to_string());
            vec![parts]
        }
        None => vec![
            vec!["code".into(), path.display().to_string()],
            #[cfg(target_os = "macos")]
            vec!["open".into(), path.display().to_string()],
            #[cfg(target_os = "linux")]
            vec!["xdg-open".into(), path.display().to_string()],
        ],
    };
    let mut last = String::new();
    for c in candidates {
        match std::process::Command::new(&c[0]).args(&c[1..]).spawn() {
            Ok(_) => return Ok(()),
            Err(e) => last = format!("{}: {e}", c[0]),
        }
    }
    Err(format!("could not open editor ({last})"))
}

#[tauri::command]
fn path_exists(path: PathBuf) -> bool {
    Path::new(&path).exists()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = Config::load().unwrap_or_default();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { config: Mutex::new(config) })
        .invoke_handler(tauri::generate_handler![
            get_config,
            set_library,
            clone_library,
            git_publication_preview,
            publish_library,
            check_library_git,
            update_library_git,
            prepare_install,
            set_agents_library,
            set_editor,
            remember_project,
            scan_library,
            search_skills,
            get_skill,
            read_skill_file,
            write_skill_file,
            update_meta,
            create_skill,
            check_hosts,
            delete_skill,
            import_skill,
            lint_skill,
            project_status,
            install_skills,
            uninstall_skill,
            diff_installed,
            sync_skill,
            adopt_skill,
            open_in_editor,
            path_exists,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
