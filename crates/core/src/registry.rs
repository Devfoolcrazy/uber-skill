//! The allowed categories and tags of a library, kept in `uber-skill.yaml` at its
//! root so they are versioned with the skills and agents they describe.
//!
//! The registry is tolerant: a value it does not list never blocks scanning,
//! importing or editing an item. It is only reported as unknown, until the user
//! adds it to the registry or maps it to an allowed value. A library without the
//! file has no registry and no unknown values.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::{Error, InputError, Result};
use crate::frontmatter::{normalize_tag, normalize_tags};
use crate::fsutil;
use crate::library::libraries;
use crate::model::{ItemKind, Skill};

pub const REGISTRY_FILE: &str = "uber-skill.yaml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Facet {
    Category,
    Tag,
}

impl Facet {
    /// Tags are case-insensitive like everywhere else; categories keep their case.
    fn normalize(self, value: &str) -> String {
        match self {
            Facet::Category => value.trim().to_string(),
            Facet::Tag => normalize_tag(value),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// What to do with the items still using a value that is being removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnUsed {
    ReplaceWith(String),
    Strip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemRef {
    pub kind: ItemKind,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueUsage {
    pub value: String,
    /// Listed in the registry. Always true when the library has no registry.
    pub known: bool,
    pub items: Vec<ItemRef>,
}

/// The registry merged with what the skills and agents actually use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryView {
    pub path: PathBuf,
    pub exists: bool,
    pub categories: Vec<ValueUsage>,
    pub tags: Vec<ValueUsage>,
}

fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.retain(|v| !v.is_empty());
    values.sort_by_key(|v| v.to_lowercase());
    values.dedup();
    values
}

impl Registry {
    pub fn path(root: &Path) -> PathBuf {
        root.join(REGISTRY_FILE)
    }

    /// None when the library has no registry file.
    pub fn load(root: &Path) -> Result<Option<Registry>> {
        let path = Registry::path(root);
        if !path.is_file() {
            return Ok(None);
        }
        let text = fsutil::read_to_string(&path)?;
        let registry: Registry = if text.trim().is_empty() {
            Registry::default()
        } else {
            serde_yaml::from_str(&text)?
        };
        Ok(Some(registry.normalized()))
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        let text = serde_yaml::to_string(&self.clone().normalized())?;
        fsutil::write_string(&Registry::path(root), &text)
    }

    fn normalized(self) -> Registry {
        Registry {
            categories: sorted(self.categories.iter().map(|c| Facet::Category.normalize(c)).collect()),
            tags: normalize_tags(self.tags),
        }
    }

    fn values(&self, facet: Facet) -> &Vec<String> {
        match facet {
            Facet::Category => &self.categories,
            Facet::Tag => &self.tags,
        }
    }

    fn values_mut(&mut self, facet: Facet) -> &mut Vec<String> {
        match facet {
            Facet::Category => &mut self.categories,
            Facet::Tag => &mut self.tags,
        }
    }

    pub fn allows(&self, facet: Facet, value: &str) -> bool {
        self.values(facet).iter().any(|v| v == value)
    }
}

fn uses(skill: &Skill, facet: Facet, value: &str) -> bool {
    match facet {
        Facet::Category => skill.category.as_deref() == Some(value),
        Facet::Tag => skill.tags.iter().any(|t| t == value),
    }
}

fn usages(facet: Facet, registry: Option<&Registry>, items: &[Skill]) -> Vec<ValueUsage> {
    let mut by_value: BTreeMap<String, Vec<ItemRef>> = BTreeMap::new();
    for value in registry.map(|r| r.values(facet).clone()).unwrap_or_default() {
        by_value.entry(value).or_default();
    }
    for item in items {
        let values: Vec<&String> = match facet {
            Facet::Category => item.category.iter().collect(),
            Facet::Tag => item.tags.iter().collect(),
        };
        for value in values {
            by_value.entry(value.clone()).or_default().push(ItemRef {
                kind: item.kind,
                id: item.id.clone(),
            });
        }
    }
    let mut out: Vec<ValueUsage> = by_value
        .into_iter()
        .map(|(value, items)| ValueUsage {
            known: registry.is_none_or(|r| r.allows(facet, &value)),
            value,
            items,
        })
        .collect();
    out.sort_by_key(|u| u.value.to_lowercase());
    out
}

/// The registry and the use of every value, across skills and agents.
pub fn view(cfg: &Config) -> Result<RegistryView> {
    let root = cfg.library_path()?;
    let registry = Registry::load(&root)?;
    let mut items = Vec::new();
    for lib in libraries(cfg)? {
        items.extend(lib.scan()?.skills);
    }
    Ok(RegistryView {
        path: Registry::path(&root),
        exists: registry.is_some(),
        categories: usages(Facet::Category, registry.as_ref(), &items),
        tags: usages(Facet::Tag, registry.as_ref(), &items),
    })
}

/// Create the registry from the values already in use.
pub fn init(cfg: &Config) -> Result<RegistryView> {
    let root = cfg.library_path()?;
    if Registry::load(&root)?.is_none() {
        let current = view(cfg)?;
        let values = |usages: Vec<ValueUsage>| usages.into_iter().map(|u| u.value).collect();
        Registry {
            categories: values(current.categories),
            tags: values(current.tags),
        }
        .save(&root)?;
    }
    view(cfg)
}

fn checked(facet: Facet, value: &str) -> Result<String> {
    let value = facet.normalize(value);
    // Tags are stored comma-separated in the frontmatter.
    if value.is_empty() || value.chars().any(char::is_control) || (facet == Facet::Tag && value.contains(',')) {
        return Err(Error::InvalidInput(InputError::RegistryValue(value)));
    }
    Ok(value)
}

/// Allow a value, creating the registry if needed. Also how an unknown value
/// found in the library gets accepted.
pub fn add(cfg: &Config, facet: Facet, value: &str) -> Result<RegistryView> {
    let root = cfg.library_path()?;
    let value = checked(facet, value)?;
    let mut registry = Registry::load(&root)?.unwrap_or_default();
    registry.values_mut(facet).push(value);
    registry.save(&root)?;
    view(cfg)
}

/// Replace `from` by `to` (or drop it) in every item using it.
/// Rewrite every item using `from`, then refresh the index that lists them.
fn rewrite_items(cfg: &Config, facet: Facet, from: &str, to: Option<&str>) -> Result<()> {
    rewrite_frontmatters(cfg, facet, from, to)?;
    crate::index::update(cfg)?;
    Ok(())
}

fn rewrite_frontmatters(cfg: &Config, facet: Facet, from: &str, to: Option<&str>) -> Result<()> {
    for lib in libraries(cfg)? {
        for item in lib.scan()?.skills.iter().filter(|s| uses(s, facet, from)) {
            match facet {
                Facet::Category => lib.update_meta(&item.id, None, Some(to), None, None)?,
                Facet::Tag => {
                    let tags: Vec<String> = item
                        .tags
                        .iter()
                        .filter(|t| *t != from)
                        .cloned()
                        .chain(to.map(str::to_string))
                        .collect();
                    lib.update_meta(&item.id, Some(&tags), None, None, None)?
                }
            };
        }
    }
    Ok(())
}

/// Rename a value everywhere it is used. When `from` is not in the registry this
/// maps an unknown value onto `to`; when `to` already exists the two are merged.
pub fn rename(cfg: &Config, facet: Facet, from: &str, to: &str) -> Result<RegistryView> {
    let root = cfg.library_path()?;
    let to = checked(facet, to)?;
    let from = facet.normalize(from);
    if from != to {
        rewrite_items(cfg, facet, &from, Some(&to))?;
        if let Some(mut registry) = Registry::load(&root)? {
            if registry.allows(facet, &from) {
                registry.values_mut(facet).retain(|v| *v != from);
                registry.values_mut(facet).push(to);
                registry.save(&root)?;
            }
        }
    }
    view(cfg)
}

/// Remove a value from the registry. Items still using it are only touched
/// according to an explicit `on_used`; without one, a used value is refused.
pub fn remove(cfg: &Config, facet: Facet, value: &str, on_used: Option<OnUsed>) -> Result<RegistryView> {
    let root = cfg.library_path()?;
    let value = facet.normalize(value);
    let current = view(cfg)?;
    let usages = match facet {
        Facet::Category => &current.categories,
        Facet::Tag => &current.tags,
    };
    let used = usages.iter().any(|u| u.value == value && !u.items.is_empty());
    match on_used {
        Some(OnUsed::ReplaceWith(to)) => {
            let to = checked(facet, &to)?;
            if to != value {
                rewrite_items(cfg, facet, &value, Some(&to))?;
            }
        }
        Some(OnUsed::Strip) => rewrite_items(cfg, facet, &value, None)?,
        None if used => return Err(Error::InvalidInput(InputError::RegistryValueInUse(value))),
        None => {}
    }
    if let Some(mut registry) = Registry::load(&root)? {
        registry.values_mut(facet).retain(|v| *v != value);
        registry.save(&root)?;
    }
    view(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Library;
    use std::fs;
    use tempfile::{tempdir, TempDir};

    fn library() -> (TempDir, Config) {
        let tmp = tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        fs::create_dir(root.join("skills")).unwrap();
        fs::create_dir(root.join("agents")).unwrap();
        let skills = Library::open(root.join("skills")).unwrap();
        skills
            .create(
                "review-pr",
                "Review",
                Some("review"),
                &["git".into(), "quality".into()],
                &[],
            )
            .unwrap();
        skills
            .create("commit-message", "Commit", Some("Writing"), &["git".into()], &[])
            .unwrap();
        let agents = Library::open_kind(root.join("agents"), ItemKind::Agent).unwrap();
        agents
            .create("reviewer", "Reviews", Some("review"), &["quality".into()], &[])
            .unwrap();
        (
            tmp,
            Config {
                library_path: Some(root),
                ..Config::default()
            },
        )
    }

    fn usage<'a>(usages: &'a [ValueUsage], value: &str) -> &'a ValueUsage {
        usages
            .iter()
            .find(|u| u.value == value)
            .unwrap_or_else(|| panic!("{value} missing"))
    }

    fn tags_of(cfg: &Config, kind: ItemKind, id: &str) -> Vec<String> {
        Library::open_kind(cfg.path_for(kind).unwrap(), kind)
            .unwrap()
            .get(id)
            .unwrap()
            .tags
    }

    #[test]
    fn without_a_registry_nothing_is_unknown_and_init_adopts_used_values() {
        let (_tmp, cfg) = library();
        let before = view(&cfg).unwrap();
        assert!(!before.exists);
        assert!(before.tags.iter().chain(&before.categories).all(|u| u.known));
        assert_eq!(usage(&before.tags, "quality").items.len(), 2);

        let after = init(&cfg).unwrap();
        assert!(after.exists);
        let saved = Registry::load(&cfg.library_path().unwrap()).unwrap().unwrap();
        assert_eq!(saved.tags, ["git", "quality"]);
        assert_eq!(saved.categories, ["review", "Writing"]);
    }

    #[test]
    fn unknown_values_are_reported_never_rejected() {
        let (_tmp, cfg) = library();
        add(&cfg, Facet::Tag, " Git ").unwrap();
        let current = view(&cfg).unwrap();
        assert!(usage(&current.tags, "git").known);
        assert!(!usage(&current.tags, "quality").known);
        assert!(!usage(&current.categories, "review").known);
        // Editing an item with an unknown value still works and keeps it.
        let skills = Library::open(cfg.skills_path().unwrap()).unwrap();
        skills
            .update_meta(
                "review-pr",
                Some(&["quality".into(), "brand-new".into()]),
                None,
                None,
                None,
            )
            .unwrap();
        assert_eq!(tags_of(&cfg, ItemKind::Skill, "review-pr"), ["brand-new", "quality"]);
        assert!(add(&cfg, Facet::Tag, "a, b").is_err());
    }

    #[test]
    fn rename_updates_registry_and_every_item_of_both_kinds() {
        let (_tmp, cfg) = library();
        init(&cfg).unwrap();
        let current = rename(&cfg, Facet::Tag, "quality", "Code-Quality").unwrap();
        assert!(usage(&current.tags, "code-quality").known);
        assert!(current.tags.iter().all(|u| u.value != "quality"));
        assert_eq!(tags_of(&cfg, ItemKind::Skill, "review-pr"), ["code-quality", "git"]);
        assert_eq!(tags_of(&cfg, ItemKind::Agent, "reviewer"), ["code-quality"]);
        assert_eq!(tags_of(&cfg, ItemKind::Skill, "commit-message"), ["git"]);

        let current = rename(&cfg, Facet::Category, "review", "Relecture").unwrap();
        assert_eq!(usage(&current.categories, "Relecture").items.len(), 2);
    }

    #[test]
    fn mapping_an_unknown_value_merges_it_into_an_allowed_one() {
        let (_tmp, cfg) = library();
        add(&cfg, Facet::Tag, "git").unwrap();
        let current = rename(&cfg, Facet::Tag, "quality", "git").unwrap();
        assert_eq!(current.tags.len(), 1);
        assert_eq!(usage(&current.tags, "git").items.len(), 3);
        assert_eq!(tags_of(&cfg, ItemKind::Skill, "review-pr"), ["git"]);
        assert_eq!(
            Registry::load(&cfg.library_path().unwrap()).unwrap().unwrap().tags,
            ["git"]
        );
    }

    #[test]
    fn removing_a_used_value_needs_an_explicit_choice() {
        let (_tmp, cfg) = library();
        init(&cfg).unwrap();
        assert!(matches!(
            remove(&cfg, Facet::Tag, "git", None),
            Err(Error::InvalidInput(InputError::RegistryValueInUse(_)))
        ));
        assert_eq!(tags_of(&cfg, ItemKind::Skill, "commit-message"), ["git"]);

        let current = remove(&cfg, Facet::Tag, "git", Some(OnUsed::ReplaceWith("quality".into()))).unwrap();
        assert!(current.tags.iter().all(|u| u.value != "git"));
        assert_eq!(tags_of(&cfg, ItemKind::Skill, "commit-message"), ["quality"]);

        let current = remove(&cfg, Facet::Category, "review", Some(OnUsed::Strip)).unwrap();
        assert!(current.categories.iter().all(|u| u.value != "review"));
        let skills = Library::open(cfg.skills_path().unwrap()).unwrap();
        assert_eq!(skills.get("review-pr").unwrap().category, None);

        add(&cfg, Facet::Tag, "unused").unwrap();
        assert!(remove(&cfg, Facet::Tag, "unused", None).is_ok());
    }

    #[test]
    fn lint_warns_about_unknown_values_without_failing() {
        let (_tmp, cfg) = library();
        add(&cfg, Facet::Tag, "git").unwrap();
        let registry = Registry::load(&cfg.library_path().unwrap()).unwrap().unwrap();
        let item = Library::open(cfg.skills_path().unwrap())
            .unwrap()
            .get("review-pr")
            .unwrap();
        let issues = crate::lint::lint_registry(&item, &registry);
        assert_eq!(issues.iter().map(|i| i.rule).collect::<Vec<_>>(), ["category", "tags"]);
        assert!(issues.iter().all(|i| i.severity == crate::lint::Severity::Warning));
        assert!(issues[1].message.contains("quality"));
    }

    #[test]
    fn bulk_edits_leave_the_rest_of_each_file_untouched() {
        let (_tmp, cfg) = library();
        let file = cfg.skills_path().unwrap().join("review-pr/SKILL.md");
        let text = "---\nname: review-pr\n# reviewed by hand\ndescription: >\n  Review a\n  pull request\nmetadata:\n  category: review\n  tags: git, quality\n---\nBody\n";
        fs::write(&file, text).unwrap();
        rename(&cfg, Facet::Tag, "quality", "rigor").unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            text.replace("git, quality", "git, rigor")
        );
    }
}
