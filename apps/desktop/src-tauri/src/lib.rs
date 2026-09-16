//! Tauri commands: thin wrappers over `uber_skill_core`, with a shared config state.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::State;
use uber_skill_core::{install, lint, search, Config, FileDiff, InstalledSkill, Issue, Library, LockEntry, Query, Skill, Target};

struct AppState {
    config: Mutex<Config>,
}

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn open_library(state: &AppState) -> CmdResult<Library> {
    let cfg = state.config.lock().map_err(err)?;
    let path = cfg.library_path().map_err(err)?;
    Library::open(path).map_err(err)
}

#[derive(Serialize)]
struct LibraryView {
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
    let lib = Library::open(&path).map_err(err)?;
    let mut cfg = state.config.lock().map_err(err)?;
    cfg.library_path = Some(lib.root().to_path_buf());
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
fn scan_library(state: State<AppState>) -> CmdResult<LibraryView> {
    let lib = open_library(&state)?;
    let scan = lib.scan().map_err(err)?;
    let (tags, categories) = Library::facets(&scan.skills);
    Ok(LibraryView { root: lib.root().to_path_buf(), skills: scan.skills, warnings: scan.warnings, tags, categories })
}

#[tauri::command]
fn search_skills(state: State<AppState>, query: Query) -> CmdResult<Vec<Skill>> {
    let lib = open_library(&state)?;
    let scan = lib.scan().map_err(err)?;
    Ok(search::search(&scan.skills, &query).into_iter().cloned().collect())
}

#[tauri::command]
fn get_skill(state: State<AppState>, id: String) -> CmdResult<Skill> {
    open_library(&state)?.get(&id).map_err(err)
}

#[tauri::command]
fn read_skill_file(state: State<AppState>, id: String, rel: String) -> CmdResult<String> {
    open_library(&state)?.read_file(&id, &rel).map_err(err)
}

#[tauri::command]
fn write_skill_file(state: State<AppState>, id: String, rel: String, text: String) -> CmdResult<Skill> {
    let lib = open_library(&state)?;
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
fn update_meta(state: State<AppState>, id: String, patch: MetaPatch) -> CmdResult<Skill> {
    let lib = open_library(&state)?;
    let category = if patch.set_category { Some(patch.category.as_deref()) } else { None };
    lib.update_meta(&id, patch.tags.as_deref(), category, patch.hosts.as_deref(), patch.description.as_deref()).map_err(err)
}

#[tauri::command]
fn create_skill(state: State<AppState>, id: String, description: String, category: Option<String>, tags: Vec<String>, hosts: Vec<String>) -> CmdResult<Skill> {
    open_library(&state)?.create(&id, &description, category.as_deref(), &tags, &hosts).map_err(err)
}

/// Warnings for skills whose declared hosts exclude `target` (empty = all fine).
#[tauri::command]
fn check_hosts(state: State<AppState>, ids: Vec<String>, target: Target) -> CmdResult<Vec<String>> {
    let lib = open_library(&state)?;
    let mut out = Vec::new();
    for id in ids {
        let skill = lib.get(&id).map_err(err)?;
        if let Some(w) = install::host_mismatch(&skill, &target) {
            out.push(w);
        }
    }
    Ok(out)
}

#[tauri::command]
fn delete_skill(state: State<AppState>, id: String) -> CmdResult<()> {
    open_library(&state)?.delete(&id).map_err(err)
}

#[tauri::command]
fn import_skill(state: State<AppState>, path: PathBuf, new_id: Option<String>) -> CmdResult<Skill> {
    open_library(&state)?.import(&path, new_id.as_deref()).map_err(err)
}

#[tauri::command]
fn lint_skill(state: State<AppState>, id: String) -> CmdResult<Vec<Issue>> {
    let lib = open_library(&state)?;
    let skill = lib.get(&id).map_err(err)?;
    lint::lint_dir(&skill.path).map_err(err)
}

#[tauri::command]
fn project_status(state: State<AppState>, project: PathBuf, target: Target) -> CmdResult<Vec<InstalledSkill>> {
    let lib = open_library(&state).ok();
    install::status(lib.as_ref(), &project, &target).map_err(err)
}

#[tauri::command]
fn install_skills(state: State<AppState>, ids: Vec<String>, project: PathBuf, target: Target) -> CmdResult<Vec<LockEntry>> {
    let lib = open_library(&state)?;
    let mut out = Vec::new();
    for id in ids {
        let skill = lib.get(&id).map_err(err)?;
        out.push(install::install(&skill, &project, &target).map_err(err)?);
    }
    Ok(out)
}

#[tauri::command]
fn uninstall_skill(id: String, project: PathBuf, target: Target) -> CmdResult<()> {
    install::uninstall(&id, &project, &target).map_err(err)
}

#[tauri::command]
fn diff_installed(state: State<AppState>, id: String, project: PathBuf, target: Target) -> CmdResult<Vec<FileDiff>> {
    let lib = open_library(&state)?;
    install::diff_installed(&lib, &id, &project, &target).map_err(err)
}

#[tauri::command]
fn sync_skill(state: State<AppState>, id: String, direction: String, project: PathBuf, target: Target) -> CmdResult<()> {
    let lib = open_library(&state)?;
    match direction.as_str() {
        "pull" => install::sync_to_project(&lib, &id, &project, &target).map(|_| ()).map_err(err),
        "push" => install::sync_to_library(&lib, &id, &project, &target).map(|_| ()).map_err(err),
        other => Err(format!("unknown direction {other}")),
    }
}

#[tauri::command]
fn adopt_skill(state: State<AppState>, id: String, project: PathBuf, target: Target) -> CmdResult<LockEntry> {
    let lib = open_library(&state)?;
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
