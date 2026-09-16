# Uber Skill

Gestionnaire de bibliothèque de skills pour agents (dossiers contenant un `SKILL.md`).
Le disque est la source de vérité : une bibliothèque est un simple dossier, les tags et la
catégorie vivent dans le frontmatter sous `metadata`, et l'app ne fait qu'indexer.

```
crates/core      bibliothèque Rust : scan, frontmatter, install, dérive, lint, config
crates/cli       binaire `uber-skill`
apps/desktop     app Tauri v2 + SvelteKit
```

## Concepts

- **Bibliothèque** : un dossier ; chaque sous-dossier avec un `SKILL.md` est un skill (imbrication autorisée, symlinks suivis).
- **Installer** : copie le skill dans le projet (`.claude/skills`, `.agents/skills`, `.cursor/skills`, `.github/skills` ou un dossier custom) et écrit un verrou `.uber-skill.lock.json` avec la source et le hash du contenu.
- **Dérive** : à partir du verrou, chaque skill installé est `up-to-date`, `library-updated`, `project-modified`, `conflict`, `untracked`, `missing` ou `source-missing`. On peut voir le diff, tirer la version bibliothèque (`pull`) ou remonter la version projet (`push`).
- **Harnais** : un skill est universel par défaut. S'il dépend d'un outil (frontmatter `allowed-tools`, outils MCP, sous-agents…), on le déclare dans `metadata.hosts` (`claude-code`, `codex`, `cursor`, `copilot`). L'app filtre dessus et avertit à l'installation si la cible ne correspond pas.
- **Lint** : nom = dossier, description présente et < 1024 caractères, corps non vide, liens relatifs existants, hosts connus.

Frontmatter reconnu :

```yaml
---
name: review-pr
description: Relit une pull request…
metadata:
  category: review
  tags: git, quality
  hosts: claude-code   # optionnel, seulement si le skill dépend d'un harnais
---
```

## CLI

```sh
cargo build -p uber-skill-cli
./target/debug/uber-skill config set-library ~/ma-bibliotheque
./target/debug/uber-skill list --tag git
./target/debug/uber-skill search "pull request"
./target/debug/uber-skill new review-pr -d "…" -c review --tag git
./target/debug/uber-skill tag review-pr --add quality --category review --host claude-code
./target/debug/uber-skill list --host codex   # skills utilisables dans Codex
./target/debug/uber-skill lint
./target/debug/uber-skill install review-pr commit-message -p ~/mon/projet -t claude
./target/debug/uber-skill status -p ~/mon/projet
./target/debug/uber-skill diff review-pr -p ~/mon/projet
./target/debug/uber-skill sync review-pr push -p ~/mon/projet
```

`--json` sur toutes les commandes de lecture. `--library <dir>` ou `UBER_SKILL_LIBRARY` remplace la config.
La config est dans `~/Library/Application Support/com.lefebvreremy.uber-skill/config.json` (ou `UBER_SKILL_CONFIG`).

## App de bureau

```sh
cd apps/desktop
pnpm install
pnpm tauri dev
```

Colonnes : filtres (catégories, tags) · liste avec recherche et cases à cocher · détail (aperçu markdown, éditeur ⌘S, fichiers, lint, tags & catégorie) · projet (choix du dossier et de la cible, installation des skills cochés, état de dérive, diff, pull/push, retirer).

## Tests

```sh
cargo test
cd apps/desktop && pnpm check
```

Note macOS : la licence Xcode n'étant pas acceptée sur cette machine, `.cargo/config.toml` force
`DEVELOPER_DIR` sur les Command Line Tools.
