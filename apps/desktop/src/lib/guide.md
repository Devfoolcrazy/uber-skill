# Mode d'emploi

Uber Skill gère une **bibliothèque** de skills et d'agents pour Claude Code et Codex, et leurs copies **installées** dans vos projets ou sur votre machine. Le disque est la source de vérité : la bibliothèque est un dépôt Git, chaque skill est un dossier avec un `SKILL.md`, chaque agent un fichier `.md`. L'application indexe, compare et copie ; elle n'invente aucun état caché.

## Les trois lieux

| Lieu | Ce que c'est | Où |
|---|---|---|
| **Bibliothèque** | Vos skills et agents, versionnés dans Git | le dépôt ouvert avec « Ouvrir / Cloner… » |
| **Projet** | Une copie installée pour un projet précis | `.claude/skills`, `.claude/agents`, `.agents/skills`… du projet |
| **Global** 🌐 | Une copie installée pour toute la machine, disponible dans tous les projets | `~/.claude/skills`, `~/.claude/agents`, `~/.agents/skills` |

Deux synchronisations, à ne pas confondre :

- **Bibliothèque ↔ dépôt distant** : « Publier… » envoie vos commits, « Récupérer… » reçoit ceux des autres machines. Cela ne touche jamais aux projets.
- **Bibliothèque → projet** : « Installer » copie un élément ; ensuite l'application compare la copie à la bibliothèque et signale les écarts.

## Lire les indicateurs

Dans la liste, à côté du nom d'un élément :

- **Pastille de couleur** : état de la copie dans le projet courant. Vert à jour, orange en retard ou modifié dans le projet, rouge conflit ou manquant, bleu non suivi.
- **✎ ↑ ↓** : écart avec le dépôt distant. ✎ modifications non publiées, ↑ commit local à envoyer, ↓ version plus récente à récupérer (d'après la dernière récupération).
- **🌐** : une copie est installée globalement ; sa couleur suit l'état de cette copie.
- **↻ n** : des copies sont en retard dans d'autres projets suivis.

Le bouton « Publier… » compte les éléments à publier, l'onglet « Projets » compte les copies en retard.

## Parcours courant

1. **Modifier** un skill dans l'onglet Éditer (⌘S enregistre) ou avec « Ouvrir dans l'éditeur ». Enregistrer ne publie rien.
2. **Mettre à jour les copies** : après un enregistrement, l'application propose « Mettre à jour la copie du projet » ou, dans « Installé dans », « Mettre à jour partout ». Quand la bibliothèque est à jour avec le dépôt distant, l'installation se fait sans dialogue.
3. **Publier** quand vous le décidez : « Publier… » montre les fichiers modifiés, vous choisissez lesquels commiter, et le push suit. Un push refusé garde le commit local ; « Envoyer les commits » réessaie.

## Installer

Cochez des éléments dans la liste, ou utilisez « Installer dans … » dans le détail. Choisissez la destination dans le sélecteur de la barre du haut : un projet, ou **🌐 Global** pour toute la machine. Avant de copier, l'application consulte le dépôt distant : si la bibliothèque est en retard, elle propose de la mettre à jour d'abord ; hors ligne, « Installer sans vérification » reste un choix explicite.

Le tiroir « Installés » et l'onglet « Projets » listent les copies et leurs états : « Mettre à jour », « Diff », « Remonter dans la bibliothèque » (la copie du projet devient la version de référence), « Retirer du projet… ». Une copie modifiée dans le projet n'est jamais remplacée en lot.

Pour Codex, un skill se place dans `.agents/skills` (projet) ou `~/.agents/skills` (global) ; `~/.codex` ne contient que sa configuration et ses skills système. Codex n'a pas de dossier d'agents, d'où « (pas d'agents) » sur cette cible. Un même skill installé à la fois globalement et dans un projet apparaît deux fois dans Codex, alors que Claude Code donne la priorité au projet.

Les skills déjà présents dans les dossiers globaux, installés à la main, apparaissent « non suivis » : « Lier à la bibliothèque » s'ils y existent déjà, « Importer dans la bibliothèque » sinon. Rien n'est modifié tant que vous ne choisissez pas.

## Projets

L'onglet « Projets » répond à « où ce skill est-il installé, et où est-il en retard ? ». Un projet est suivi dès qu'un élément y est installé ; « + Suivre un projet… » en ajoute un, « Retirer de la liste » cesse de le suivre sans rien supprimer. Un dossier déplacé reste listé, marqué « introuvable ». « Tout mettre à jour » ne touche que les copies simplement en retard.

## Créer, décrire, vérifier

- **+ Nouveau skill / agent** crée un brouillon à partir d'un modèle court (objectif, instructions, exemple) et l'ouvre dans l'éditeur.
- **Tags & catégorie** : les valeurs viennent du référentiel `uber-skill.yaml`, versionné avec la bibliothèque et administré dans « Tags et catégories… ». Une valeur inconnue n'est jamais refusée, seulement signalée.
- **Harnais** : un skill est universel par défaut. S'il dépend d'un outil (Claude Code, Codex…), déclarez-le dans `metadata.hosts` ; l'application avertit avant une installation vers une cible qui ne correspond pas.
- **Lint** : nom, description, corps, liens, tags. « Corriger » répare un lien cassé quand le fichier existe ailleurs dans le skill.
- **Raffiner…** envoie le `SKILL.md` à Claude (`claude -p`, sans outil) avec votre consigne, et montre un diff à accepter ou rejeter. Le nom, les tags et les autres clés du frontmatter sont verrouillés. Ni le lint ni le raffinement ne garantissent le comportement du skill : essayez-le dans un projet.

## Supprimer

- **Retirer du projet…** supprime seulement la copie installée dans un projet.
- **Supprimer de la bibliothèque…** retire l'élément de la bibliothèque, pour tous les projets à venir. Il part dans la Corbeille, et Git garde son historique s'il était commité.

## Ce qui reste hors de l'application

Les branches, fusions et conflits Git se règlent dans un outil Git externe ; l'application ne fait que des avances rapides et des pushs sans force. La commande `claude` doit être installée et connectée pour « Raffiner… ». Le CLI `uber-skill` offre les mêmes opérations en ligne de commande (`list`, `install`, `status`, `remote`, `registry`, `refine`, `lint`) ; `install -p ~` installe globalement.
