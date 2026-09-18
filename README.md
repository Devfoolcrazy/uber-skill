# Uber Skill

Gestionnaire de bibliothèque de skills (dossiers contenant un `SKILL.md`) et d'agents
(fichiers `<nom>.md` au format sous-agent Claude Code).
Le disque est la source de vérité : une bibliothèque est un simple dossier, les tags et la
catégorie vivent dans le frontmatter sous `metadata`, et l'app ne fait qu'indexer.

```
crates/core      bibliothèque Rust : scan, frontmatter, install, dérive, lint, config
crates/cli       binaire `uber-skill`
apps/desktop     app Tauri v2 + SvelteKit
```

## Concepts

- **Bibliothèque** : un dossier racine avec `skills/` et `agents/` (à défaut, les skills sont cherchés à la racine et les agents dans `_AGENTS`).
- **Skills** : chaque sous-dossier de `skills/` avec un `SKILL.md` (imbrication autorisée, symlinks suivis, dossiers `_xxx` ignorés).
- **Agents** : chaque fichier `.md` avec frontmatter dans `agents/` (ou le dossier configuré avec `config set-agents`). Ils s'installent dans `.claude/agents/<nom>.md` (cible Claude Code uniquement) avec la même mécanique de verrou et de dérive.
- **Installer** : copie le skill dans le projet (`.claude/skills`, `.agents/skills`, `.cursor/skills`, `.github/skills` ou un dossier custom) et écrit un verrou `.uber-skill.lock.json` avec la source et le hash du contenu.
- **Dérive** : à partir du verrou, chaque skill installé est `up-to-date`, `library-updated`, `project-modified`, `conflict`, `untracked`, `missing` ou `source-missing`. On peut voir le diff, tirer la version bibliothèque (`pull`) ou remonter la version projet (`push`).
- **Harnais** : un skill est universel par défaut. S'il dépend d'un outil (frontmatter `allowed-tools`, outils MCP, sous-agents…), on le déclare dans `metadata.hosts` (`claude-code`, `codex`, `cursor`, `copilot`). L'app filtre dessus et avertit à l'installation si la cible ne correspond pas.
- **Référentiel** : `uber-skill.yaml` à la racine de la bibliothèque liste les catégories et tags autorisés, communs aux skills et aux agents, et versionnés avec eux. Il est tolérant : une valeur absente du référentiel ne bloque ni le scan, ni l'import, ni l'édition ; elle est seulement signalée comme inconnue. Sans ce fichier, toutes les valeurs sont acceptées.
- **Lint** : nom = dossier, description présente et < 1024 caractères, corps non vide, liens relatifs existants, hosts connus, catégorie et tags présents dans le référentiel (avertissement).

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

# Agents : mêmes commandes avec --kind agent (ou -k agent)
./target/debug/uber-skill -k agent list
./target/debug/uber-skill -k agent install coder reviewer -p ~/mon/projet
./target/debug/uber-skill -k agent status -p ~/mon/projet

# Référentiel de catégories et de tags (uber-skill.yaml)
./target/debug/uber-skill registry show                      # valeurs autorisées, inconnues, et leur usage
./target/debug/uber-skill registry init                      # crée le référentiel à partir des valeurs utilisées
./target/debug/uber-skill registry add tag quality
./target/debug/uber-skill registry rename tag quality rigor  # met aussi à jour les skills et agents
./target/debug/uber-skill registry remove tag git --replace-with rigor   # ou --strip

# Dépôt Git de la bibliothèque
./target/debug/uber-skill remote status                      # fetch + commits et fichiers en attente
./target/debug/uber-skill remote update                      # avance rapide uniquement
./target/debug/uber-skill remote publish skills/review-pr/SKILL.md -m "Clarifie review-pr"
./target/debug/uber-skill remote publish --all -m "…"        # tous les fichiers modifiés
./target/debug/uber-skill remote publish                     # renvoie les commits locaux en attente
```

`--json` sur toutes les commandes de lecture. `--library <dir>` ou `UBER_SKILL_LIBRARY` remplace la config.
La config est dans `~/Library/Application Support/com.lefebvreremy.uber-skill/config.json` (ou `UBER_SKILL_CONFIG`).

## App de bureau

```sh
cd apps/desktop
pnpm install
pnpm tauri dev
```

Barre du haut : bascule Skills / Agents, sélecteur de projet et de cible, bouton « Installés » avec badge de dérive, réglages.
Colonnes : filtres (catégories, tags, harnais) · liste avec recherche et cases à cocher · détail (aperçu markdown, éditeur ⌘S, fichiers, lint, tags & catégorie).
Barre flottante quand des éléments sont cochés : installation dans le projet courant. Tiroir « Installés » : état de dérive, diff, pull/push, retirer.

### Deux synchronisations distinctes

- **Bibliothèque ↔ dépôt distant** : **Publier…** envoie vos modifications, **Récupérer…** reçoit celles des autres. Cela ne touche jamais aux projets.
- **Bibliothèque → projet** : l’installation copie un skill ou un agent dans un projet. Le tiroir « Installés » compare ensuite cette copie à la bibliothèque. Modifier un skill dans la bibliothèque met donc la copie du projet « en retard », qu’il soit publié ou non ; **Mettre à jour la copie du projet** la rafraîchit.

### Ouvrir une bibliothèque Git

Dans la barre latérale, **Ouvrir / Cloner…** propose deux parcours :

- **Dépôt local** : sélectionner la racine d’un dépôt Git déjà cloné (les worktrees sont également acceptés).
- **Cloner un dépôt** : saisir l’URL, choisir un dossier parent et nommer le nouveau dossier à créer. Une destination déjà existante est refusée, même si elle est vide.

Git doit être installé et accessible à l’application. Pour un dépôt privé, les accès Git
(clé SSH ou gestionnaire d’identifiants HTTPS) doivent déjà être configurés sur la machine ;
l’application ne demande pas de mot de passe dans un terminal.

L’ouverture d’un dépôt charge ses skills et ses agents et remplace l’ancien chemin d’agents
personnalisé. Le choix est enregistré pour le prochain lancement. Un échec de validation ou
de clonage conserve la bibliothèque précédente. Les bibliothèques de dossiers configurées
avant cette évolution restent lisibles au démarrage.

### Publier les modifications Git

Le bouton **Publier…** ouvre l’aperçu des changements enregistrés sur disque, y compris
ceux effectués hors de l’application. Cocher les fichiers à inclure, consulter leurs différences,
adapter le message de commit prérempli puis cliquer sur **Commit et push**. Un renommage
apparaît comme une suppression et un ajout : sélectionner les deux pour le publier entièrement.
Les modifications non enregistrées dans l’éditeur ne sont pas incluses ; un rappel est affiché.

La publication utilise la branche courante et sa branche distante de suivi déjà configurée.
Les autres fichiers préparés dans Git restent exclus du commit et conservent leur préparation.
Les commits locaux déjà en attente sont indiqués, car ils seront également envoyés.
L’aperçu n’altère pas la préparation existante ; un changement depuis l’aperçu impose de l’actualiser.

Si le commit réussit mais que le push échoue, le commit reste local et **Envoyer les commits**
permet de réessayer sans nouveau commit. Aucun push forcé n’est effectué. L’identité de commit,
les hooks et la signature utilisent la configuration Git de la machine. Un commit refusé laisse
les fichiers sélectionnés préparés dans Git et n’est pas suivi d’un push.

Les branches sans suivi distant, HEAD détachée, opérations Git en cours, conflits et changements
de sous-modules doivent être traités dans un outil Git externe. L’aperçu de publication utilise
le dernier état distant connu localement.

### Récupérer et vérifier avant installation

**Récupérer…** vérifie la branche distante configurée et affiche les commits à récupérer,
les commits locaux à publier et les fichiers modifiés. **Mettre à jour la bibliothèque** applique
uniquement une avance rapide, sans fusion ni stash automatique. Git conserve les modifications
locales compatibles et refuse la mise à jour si elles risquent d’être écrasées, y compris les fichiers
ignorés. Une divergence ou une opération Git en cours doit être résolue dans un outil Git externe.
Les modifications non enregistrées dans l’éditeur empêchent la mise à jour jusqu’à leur enregistrement.

Chaque installation depuis l’application (unitaire, groupée, skill ou agent, réinstallation ou mise
à jour depuis le tiroir « Installés ») ouvre le même contrôle :

- Bibliothèque à jour avec le dépôt distant : une seule action, **Installer dans le projet**. Elle copie les fichiers présents dans la bibliothèque, y compris un brouillon non publié.
- Bibliothèque en retard : **Mettre à jour la bibliothèque puis installer** récupère la version distante, recharge les éléments choisis et les installe ; **Installer la version actuelle de la bibliothèque** reste possible.
- Si le réseau ou les accès Git empêchent la vérification, **Installer sans vérification** reste un choix explicite.
- Si la mise à jour échoue, aucune installation ne suit automatiquement.

Les changements de source depuis le contrôle imposent une nouvelle vérification avant copie.
Le verrou conserve l’état de la source à l’installation (version publiée, brouillon local, commit
non publié ou fraîcheur non vérifiée), affiché dans le tiroir « Installés ». Les anciens verrous
restent lisibles. Le hash de contenu continue de servir à détecter les écarts projet/bibliothèque.

Côté CLI, `remote status`, `remote update` et `remote publish` appliquent les mêmes règles que
l’application (suivi distant configuré, avance rapide uniquement, aucun push forcé). `install` reste
une copie locale sans vérification de fraîcheur : lancer `remote status` avant si nécessaire. Remonter une copie projet vers la bibliothèque reste un enregistrement
local ; sa publication Git est une action séparée.

### Créer un skill ou un agent

**+ Nouveau skill** (ou **+ Nouvel agent**) demande un identifiant, une description et, facultativement, une
catégorie et des tags du référentiel. L’élément est créé à partir d’un modèle intégré, distinct pour les
skills et les agents, à la structure courte : objectif, instructions, exemple d’utilisation. Il s’ouvre
aussitôt dans l’éditeur. Rien n’est écrasé : un nom déjà pris est refusé, même par un dossier ou un fichier
qui n’est pas un skill ou un agent. Le brouillon reste local jusqu’à **Publier…**. La commande CLI `new`
utilise les mêmes modèles.

### Tags et catégories

**Tags et catégories…** (barre latérale, ou ⚙ › Bibliothèque) administre le référentiel : ajouter une
valeur, la renommer partout où elle est utilisée (un nom déjà existant fusionne les deux valeurs), la
supprimer. Supprimer une valeur encore utilisée demande un choix explicite : la remplacer par une autre ou
la retirer des éléments concernés. Les valeurs inconnues, trouvées dans des skills ou agents mais absentes
du référentiel, peuvent être ajoutées au référentiel ou remplacées par une valeur autorisée ; tant
qu’aucun choix n’est fait, elles sont conservées telles quelles.

Dès que le référentiel existe, l’éditeur de métadonnées et le formulaire de création proposent des
sélecteurs de valeurs autorisées ; les valeurs inconnues déjà présentes sur un élément restent visibles et
peuvent être retirées. Les renommages et remplacements ne réécrivent que les lignes `tags` et `category`
du frontmatter lorsque c’est possible, pour garder des différences Git lisibles. Tout est enregistré sur
disque ; la publication reste l’action **Publier…**. Les opérations qui réécrivent des skills ou agents
sont désactivées tant que l’éditeur contient des modifications non enregistrées.

## Erreurs et libellés

Le `core` ne produit aucun texte destiné à l’utilisateur : une erreur porte un code stable
(`git-stale`, `blocked.diverged`, `input.clone-url`…), un message anglais pour les logs et le CLI, et des
détails jamais traduits (chemin, identifiant, sortie de Git). L’application traduit les codes dans
`apps/desktop/src/lib/errors.ts` ; un test du `core` échoue si un code n’y a pas de libellé.

## Tests

```sh
cargo test                                    # core, sur de vrais dépôts Git temporaires
cd apps/desktop && pnpm check && pnpm test    # types, puis store et dialogues Git (vitest, passerelle Tauri simulée)
```

`cargo fmt --all` avant de commiter (largeur 120, voir `rustfmt.toml`).

Note macOS : la licence Xcode n'étant pas acceptée sur cette machine, `.cargo/config.toml` force
`DEVELOPER_DIR` sur les Command Line Tools.
