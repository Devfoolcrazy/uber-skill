use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use uber_skill_core::{install, lint, search, Config, DriftState, ItemKind, Library, Query, Target};

#[derive(Parser)]
#[command(name = "uber-skill", version, about = "Manage a library of agent skills")]
struct Cli {
    /// Library root (overrides the configured one)
    #[arg(long, global = true, env = "UBER_SKILL_LIBRARY")]
    library: Option<PathBuf>,
    /// Output JSON instead of text
    #[arg(long, global = true)]
    json: bool,
    /// Work on skills (default) or agents
    #[arg(short, long, global = true, default_value = "skill", value_parser = ["skill", "agent"])]
    kind: String,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Args, Clone)]
struct ProjectArgs {
    /// Project root (default: current directory)
    #[arg(short, long, default_value = ".")]
    project: PathBuf,
    /// Install target: claude | agents | cursor | copilot | <custom relative dir>
    #[arg(short, long, default_value = "claude")]
    target: String,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show or set configuration
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },
    /// List skills in the library
    List {
        #[arg(long)]
        tag: Vec<String>,
        #[arg(long)]
        category: Option<String>,
        /// Only skills usable on this host (claude-code, codex, cursor, copilot…)
        #[arg(long)]
        host: Option<String>,
    },
    /// Search skills by text (name, tags, description)
    Search {
        query: String,
        #[arg(long)]
        tag: Vec<String>,
        #[arg(long)]
        category: Option<String>,
        #[arg(long)]
        host: Option<String>,
    },
    /// Show one skill
    Show { id: String },
    /// Create a new skill in the library
    New {
        id: String,
        #[arg(short, long, default_value = "")]
        description: String,
        #[arg(short, long)]
        category: Option<String>,
        #[arg(long)]
        tag: Vec<String>,
        /// Host this skill depends on (repeatable); omit for host-agnostic skills
        #[arg(long)]
        host: Vec<String>,
    },
    /// Set tags / category / hosts / description
    Tag {
        id: String,
        #[arg(long)]
        add: Vec<String>,
        #[arg(long)]
        remove: Vec<String>,
        #[arg(long)]
        set: Vec<String>,
        #[arg(long)]
        category: Option<String>,
        #[arg(long)]
        clear_category: bool,
        /// Add a host dependency (claude-code, codex, cursor, copilot…)
        #[arg(long)]
        host: Vec<String>,
        #[arg(long)]
        remove_host: Vec<String>,
        /// Make the skill host-agnostic again
        #[arg(long)]
        clear_hosts: bool,
        #[arg(long)]
        description: Option<String>,
    },
    /// Import an external skill folder into the library
    Import {
        path: PathBuf,
        #[arg(long)]
        r#as: Option<String>,
    },
    /// Lint one skill or the whole library
    Lint { id: Option<String> },
    /// Install skills into a project
    Install {
        ids: Vec<String>,
        #[command(flatten)]
        project: ProjectArgs,
    },
    /// Remove installed skills from a project
    Uninstall {
        ids: Vec<String>,
        #[command(flatten)]
        project: ProjectArgs,
    },
    /// Show installed skills and their drift state
    Status {
        #[command(flatten)]
        project: ProjectArgs,
    },
    /// Diff between the library copy and the installed copy
    Diff {
        id: String,
        #[command(flatten)]
        project: ProjectArgs,
    },
    /// Sync a skill: pull (library -> project) or push (project -> library)
    Sync {
        id: String,
        #[arg(value_parser = ["pull", "push"])]
        direction: String,
        #[command(flatten)]
        project: ProjectArgs,
    },
    /// Record an untracked installed skill as coming from the library
    Adopt {
        id: String,
        #[command(flatten)]
        project: ProjectArgs,
    },
}

#[derive(Subcommand)]
enum ConfigCmd {
    Show,
    /// Set the library root (skills in <root>/skills, agents in <root>/agents)
    SetLibrary {
        path: PathBuf,
    },
    /// Set a separate agents folder (default: <root>/agents)
    SetAgents {
        path: PathBuf,
    },
    /// Set the external editor command (e.g. `code`)
    SetEditor {
        command: String,
    },
}

fn kind_of(cli: &Cli) -> ItemKind {
    ItemKind::parse(&cli.kind).unwrap_or_default()
}

fn open_library(cli: &Cli) -> Result<Library> {
    let kind = kind_of(cli);
    let path = match &cli.library {
        Some(p) => p.clone(),
        None => Config::load()?
            .path_for(kind)
            .context("no library configured: run `uber-skill config set-library <path>` or pass --library")?,
    };
    Library::open_kind(&path, kind).with_context(|| format!("opening {} library {}", kind.label(), path.display()))
}

fn print_json<T: serde::Serialize>(v: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.cmd {
        Cmd::Config { cmd } => match cmd {
            ConfigCmd::Show => {
                let cfg = Config::load()?;
                if cli.json {
                    return print_json(&cfg);
                }
                println!(
                    "config file: {}",
                    uber_skill_core::config::config_path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default()
                );
                println!(
                    "library:     {}",
                    cfg.library_path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "(not set)".into())
                );
                println!(
                    "skills:      {}",
                    cfg.skills_path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| "(not set)".into())
                );
                println!(
                    "agents:      {}",
                    cfg.agents_path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| "(not set)".into())
                );
                println!("editor:      {}", cfg.editor_command.as_deref().unwrap_or("(default)"));
                for p in &cfg.recent_projects {
                    println!("project:     {} [{}]", p.path.display(), p.target.label());
                }
            }
            ConfigCmd::SetLibrary { path } => {
                let path = path.canonicalize().with_context(|| format!("{}", path.display()))?;
                Library::open(&path)?;
                let mut cfg = Config::load()?;
                cfg.library_path = Some(path.clone());
                cfg.save()?;
                println!("library set to {}", path.display());
            }
            ConfigCmd::SetAgents { path } => {
                let path = path.canonicalize().with_context(|| format!("{}", path.display()))?;
                Library::open_kind(&path, ItemKind::Agent)?;
                let mut cfg = Config::load()?;
                cfg.agents_path = Some(path.clone());
                cfg.save()?;
                println!("agents library set to {}", path.display());
            }
            ConfigCmd::SetEditor { command } => {
                let mut cfg = Config::load()?;
                cfg.editor_command = Some(command.clone());
                cfg.save()?;
                println!("editor set to {command}");
            }
        },
        Cmd::List { tag, category, host }
        | Cmd::Search {
            tag, category, host, ..
        } => {
            let lib = open_library(&cli)?;
            let scan = lib.scan()?;
            let text = match &cli.cmd {
                Cmd::Search { query, .. } => Some(query.clone()),
                _ => None,
            };
            let q = Query {
                text,
                tags: tag.clone(),
                category: category.clone(),
            };
            let host_target = host.as_deref().map(Target::parse);
            let hits: Vec<_> = search::search(&scan.skills, &q)
                .into_iter()
                .filter(|s| host_target.as_ref().map(|t| t.accepts_hosts(&s.hosts)).unwrap_or(true))
                .collect();
            if cli.json {
                return print_json(&hits);
            }
            for s in &hits {
                let tags = if s.tags.is_empty() {
                    String::new()
                } else {
                    format!("  [{}]", s.tags.join(", "))
                };
                let cat = s.category.as_deref().map(|c| format!("  ({c})")).unwrap_or_default();
                let hosts = if s.hosts.is_empty() {
                    String::new()
                } else {
                    format!("  hosts: {}", s.hosts.join(", "))
                };
                println!("{:<32}{cat}{tags}{hosts}", s.id);
                println!("    {}", truncate(&s.description, 110));
            }
            for w in &scan.warnings {
                eprintln!("warning: {}: {}", w.path.display(), w.message);
            }
            eprintln!("{} {}(s)", hits.len(), kind_of(&cli).label());
        }
        Cmd::Show { id } => {
            let lib = open_library(&cli)?;
            let s = lib.get(id)?;
            if cli.json {
                return print_json(&s);
            }
            println!("id:          {}", s.id);
            println!("name:        {}", s.name);
            println!("description: {}", s.description);
            println!("category:    {}", s.category.as_deref().unwrap_or("-"));
            println!(
                "tags:        {}",
                if s.tags.is_empty() {
                    "-".into()
                } else {
                    s.tags.join(", ")
                }
            );
            println!(
                "hosts:       {}",
                if s.hosts.is_empty() {
                    "any".into()
                } else {
                    s.hosts.join(", ")
                }
            );
            println!("path:        {}", s.path.display());
            println!("hash:        {}", &s.hash[..12]);
            for (k, v) in &s.extra {
                println!("{k}: {v}");
            }
            println!("files:");
            for f in &s.files {
                println!("  {f}");
            }
        }
        Cmd::New {
            id,
            description,
            category,
            tag,
            host,
        } => {
            let lib = open_library(&cli)?;
            let s = lib.create(id, description, category.as_deref(), tag, host)?;
            println!("created {}", s.path.display());
        }
        Cmd::Tag {
            id,
            add,
            remove,
            set,
            category,
            clear_category,
            host,
            remove_host,
            clear_hosts,
            description,
        } => {
            let lib = open_library(&cli)?;
            let current = lib.get(id)?;
            let mut tags: Vec<String> = if set.is_empty() {
                current.tags.clone()
            } else {
                set.clone()
            };
            tags.extend(add.iter().cloned());
            tags.retain(|t| !remove.iter().any(|r| r.eq_ignore_ascii_case(t)));
            let cat = if *clear_category {
                Some(None)
            } else {
                category.as_deref().map(Some)
            };
            let mut hosts: Vec<String> = if *clear_hosts {
                Vec::new()
            } else {
                current.hosts.clone()
            };
            hosts.extend(host.iter().cloned());
            hosts.retain(|h| !remove_host.iter().any(|r| r.eq_ignore_ascii_case(h)));
            for h in &hosts {
                if !Target::is_known_host(h) {
                    eprintln!("warning: unknown host `{h}` (known: claude-code, codex, agents, amp, cursor, copilot)");
                }
            }
            let s = lib.update_meta(id, Some(&tags), cat, Some(&hosts), description.as_deref())?;
            println!(
                "{}: category={} tags={} hosts={}",
                s.id,
                s.category.as_deref().unwrap_or("-"),
                s.tags.join(", "),
                if s.hosts.is_empty() {
                    "any".into()
                } else {
                    s.hosts.join(", ")
                }
            );
        }
        Cmd::Import { path, r#as } => {
            let lib = open_library(&cli)?;
            let s = lib.import(path, r#as.as_deref())?;
            println!("imported {} -> {}", s.id, s.path.display());
        }
        Cmd::Lint { id } => {
            let lib = open_library(&cli)?;
            let skills = match id {
                Some(id) => vec![lib.get(id)?],
                None => lib.scan()?.skills,
            };
            let mut total_errors = 0;
            let mut report = Vec::new();
            for s in &skills {
                let issues = lint::lint_item(&s.path, s.kind)?;
                total_errors += issues.iter().filter(|i| i.severity == lint::Severity::Error).count();
                if cli.json {
                    report.push(serde_json::json!({ "id": s.id, "issues": issues }));
                    continue;
                }
                if issues.is_empty() {
                    println!("{}: ok", s.id);
                    continue;
                }
                println!("{}:", s.id);
                for i in &issues {
                    println!(
                        "  {:<7} {:<12} {}",
                        format!("{:?}", i.severity).to_lowercase(),
                        i.rule,
                        i.message
                    );
                }
            }
            if cli.json {
                print_json(&report)?;
            }
            if total_errors > 0 {
                std::process::exit(1);
            }
        }
        Cmd::Install { ids, project } => {
            if ids.is_empty() {
                bail!("give at least one skill id");
            }
            let lib = open_library(&cli)?;
            let (root, target) = project_root(project)?;
            let dir = target.dir_for(lib.kind(), &root)?;
            for id in ids {
                let s = lib.get(id)?;
                if let Some(w) = install::host_mismatch(&s, &target) {
                    eprintln!("warning: {w}");
                }
                install::install(&s, &root, &target)?;
                println!(
                    "installed {} -> {}",
                    id,
                    install::installed_path(&dir, id, s.kind).display()
                );
            }
            remember(&root, &target)?;
        }
        Cmd::Uninstall { ids, project } => {
            let (root, target) = project_root(project)?;
            for id in ids {
                install::uninstall(id, &root, &target, kind_of(&cli))?;
                println!("removed {id}");
            }
        }
        Cmd::Status { project } => {
            let lib = open_library(&cli).ok();
            let (root, target) = project_root(project)?;
            let kind = kind_of(&cli);
            let st = install::status(lib.as_ref(), &root, &target, kind)?;
            if cli.json {
                return print_json(&st);
            }
            if st.is_empty() {
                match target.dir_for(kind, &root) {
                    Ok(d) => println!("no {}s in {}", kind.label(), d.display()),
                    Err(e) => println!("{e}"),
                }
            }
            for s in &st {
                let label = match s.state {
                    DriftState::UpToDate => "up-to-date",
                    DriftState::LibraryUpdated => "library-updated (run: sync pull)",
                    DriftState::ProjectModified => "project-modified (run: sync push)",
                    DriftState::Conflict => "CONFLICT (run: diff)",
                    DriftState::Untracked => "untracked (run: adopt)",
                    DriftState::Missing => "missing",
                    DriftState::SourceMissing => "source-missing",
                };
                println!("{:<32} {label}", s.id);
            }
        }
        Cmd::Diff { id, project } => {
            let lib = open_library(&cli)?;
            let (root, target) = project_root(project)?;
            let diffs = install::diff_installed(&lib, id, &root, &target)?;
            if cli.json {
                return print_json(&diffs);
            }
            if diffs.is_empty() {
                println!("no differences");
            }
            for d in &diffs {
                println!("== {} ({})", d.file, d.kind);
                print!("{}", d.unified);
            }
        }
        Cmd::Sync { id, direction, project } => {
            let lib = open_library(&cli)?;
            let (root, target) = project_root(project)?;
            match direction.as_str() {
                "pull" => {
                    install::sync_to_project(&lib, id, &root, &target)?;
                    println!("pulled {id} from library into project");
                }
                _ => {
                    install::sync_to_library(&lib, id, &root, &target)?;
                    println!("pushed {id} from project into library");
                }
            }
        }
        Cmd::Adopt { id, project } => {
            let lib = open_library(&cli)?;
            let (root, target) = project_root(project)?;
            install::adopt(&lib, id, &root, &target)?;
            println!("adopted {id}");
        }
    }
    Ok(())
}

fn project_root(p: &ProjectArgs) -> Result<(PathBuf, Target)> {
    let root = p
        .project
        .canonicalize()
        .with_context(|| format!("project {}", p.project.display()))?;
    Ok((root, Target::parse(&p.target)))
}

fn remember(root: &Path, target: &Target) -> Result<()> {
    let mut cfg = Config::load()?;
    cfg.remember_project(root.to_path_buf(), target.clone());
    cfg.save().ok();
    Ok(())
}

fn truncate(s: &str, n: usize) -> String {
    let s = s.replace('\n', " ");
    if s.chars().count() <= n {
        s
    } else {
        let mut t: String = s.chars().take(n - 1).collect();
        t.push('…');
        t
    }
}
