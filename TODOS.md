# Fonctionnalités à spécifier

## Suivi de l’implémentation

Avancer étape par étape : chaque livraison doit être testée par l’utilisateur avant de commencer la suivante. Consigner les retours UI/UX ci-dessous et les traiter lors de la passe finale, une fois le fonctionnel posé.

1. **Ouverture d’une bibliothèque Git locale et clonage depuis une URL : implémentés et validés par l’utilisateur.** Interface « Ouvrir / Cloner… » commune aux skills et agents, validation de la racine Git, destination de clonage nouvelle uniquement, réinitialisation de l’ancien chemin d’agents personnalisé, choix conservé au prochain lancement. Les bibliothèques préexistantes sans Git restent lisibles au démarrage.
2. **Publication Git : implémentée et validée par l’utilisateur.** Aperçu, sélection par fichier, message prérempli modifiable, commit/push explicite et nouvel essai du push sans commit supplémentaire.
3. **Récupération Git et vérification de fraîcheur avant installation : implémentées et validées par l’utilisateur.** Fetch de la branche distante configurée, compteur de commits locaux/distants, mise à jour en avance rapide et choix explicite d’une installation locale. Contrôle commun aux installations unitaires, groupées, agents et réinstallations depuis le tiroir.
4. **Référentiel de tags/catégories et administration : implémentés et validés par l’utilisateur.** Fichier `uber-skill.yaml` versionné à la racine, commun aux skills et agents, tolérant aux valeurs inconnues ; page « Tags et catégories » (ajout, renommage partout, suppression avec remplacement ou retrait explicite, mise en correspondance des valeurs inconnues) ; sélecteurs dans l’éditeur et le formulaire de création ; avertissements de lint ; commandes CLI `registry`.
5. **Création de skills/agents avec templates intégrés : implémentée et validée par l’utilisateur.** Modèles distincts dans `crates/core/src/templates/`, refus d’écraser quoi que ce soit, ouverture immédiate du brouillon dans l’éditeur.
6. **Raffinement assisté du `SKILL.md` : implémenté et validé par l’utilisateur.** `claude -p` sans outil ni session, modèle par défaut du CLI ; la proposition peut modifier le corps et la description, le reste du frontmatter est verrouillé ; diff, puis acceptation ou rejet explicites.
7. **Passe UI/UX : réalisée et validée par l’utilisateur.** Constats de lint et avertissements de scan rédigés en français à partir de codes, correction en un clic d’un lien cassé et ajout au référentiel depuis l’onglet Lint, installation directe quand il n’y a rien à décider.
8. Évolutions à venir : voir la section « Évolutions à venir » en fin de document. À traiter une par une, avec le même circuit (décisions, implémentation, test utilisateur).

### Test utilisateur de l’étape 1

Vérifications effectuées : `cargo test` (13 tests réussis, dont 3 tests Git sur des dépôts temporaires), `pnpm check` (aucune erreur ni avertissement), parcours d’interface automatisés avec passerelle Tauri simulée (ouverture, erreurs, clonage, nouvelle tentative, fermeture). Le clonage distant avec les identifiants réels reste à vérifier par l’utilisateur.

- Ouvrir la racine d’un dépôt local contenant `skills/` et `agents/`, puis vérifier les deux onglets et la persistance du choix après redémarrage.
- Cloner un dépôt accessible dans un nouveau dossier, puis vérifier le chargement des contenus.
- Essayer un dossier sans Git, une destination de clonage existante et une URL inaccessible : vérifier l’erreur et la conservation de la bibliothèque précédente.
- Si un fichier a des modifications non enregistrées dans l’éditeur, vérifier la confirmation avant changement de bibliothèque.

### Retours UI/UX à traiter lors de la passe finale

- Retour utilisateur (test de l’étape 4) : confusion entre la publication Git et la mise à jour de la copie du projet ; après « Publier… », l’élément restait « Bibliothèque plus récente » et le contrôle d’installation ne proposait que « Installer la version locale », « Mettre à jour puis installer » restant grisé. Traité : libellés distinguant les deux synchronisations, une seule action « Installer dans le projet » quand la bibliothèque est à jour, mise à jour de la bibliothèque proposée seulement quand elle est en retard. Traité aussi, à la demande de l’utilisateur : après l’enregistrement d’un élément installé dans le projet courant, le détail propose directement « Mettre à jour la copie du projet » et la notification le signale. Dialogue évité quand il n’y a rien à décider : traité lors de la passe UI/UX.
- Retour utilisateur (test de l’étape 6) : après la modification d’un skill non installé dans le projet courant, rien n’indiquait l’écart avec le dépôt distant. Traité : indicateur par élément (●/↑/↓) dans la liste et le détail, compteur sur « Publier… », à partir des données Git locales.
- Retour utilisateur : un clic sur un lien relatif de l’aperçu (`cheatsheet.md`) menait à une page 404 sans retour possible, alors que le lien était valide. Traité : les liens de l’aperçu ne naviguent plus (fichier du skill ouvert sur place, lien web dans le navigateur, lien cassé signalé), page d’erreur avec « Revenir à la bibliothèque » en filet de sécurité, lint des liens étendu à tous les fichiers Markdown du skill avec suggestion. Correction en un clic ajoutée lors de la passe UI/UX.
- Retour utilisateur : en voulant retirer un élément d’un projet, le bouton « Supprimer » du détail l’a supprimé de la bibliothèque. Traité : « Retirer du projet… » ajouté dans le détail, bouton renommé « Supprimer de la bibliothèque… », confirmations natives qui disent ce qui est détruit et orientent vers la bonne action, suppression déplacée dans la Corbeille au lieu d’être définitive.
- Relevé pendant l’étape 4 : les messages de lint et les avertissements de scan étaient rédigés en anglais dans le `core`. Traité lors de la passe UI/UX (codes et arguments, libellés dans `lint.ts`).

### Test utilisateur de l’étape 7

Vérifications effectuées : suite Rust complète (53 tests, dont l’application des corrections de liens et la présence d’un libellé français pour chaque constat de lint), `pnpm check` sans erreur ni avertissement, 52 tests d’interface. L’interface n’a pas été essayée dans l’application réelle.

- Onglet Lint d’un skill : vérifier que les constats et les niveaux (Erreur, Avertissement, Info) sont en français.
- Casser un lien à la main dans un `SKILL.md` (par exemple `[x](ch01.md)` alors que le fichier est dans `chapters/`) : le constat doit proposer le bon chemin et « Corriger » doit réparer le lien sans toucher au reste du fichier. Vérifier le résultat dans « Publier… ».
- Sur un tag inconnu, « Ajouter au référentiel » depuis l’onglet Lint : le constat disparaît et le tag apparaît dans « Tags et catégories… ».
- Mettre deux skills avec le même identifiant dans la bibliothèque, ou un `SKILL.md` au frontmatter invalide : l’avertissement de la barre latérale doit être en français.
- Installer ou mettre à jour la copie d’un projet avec une bibliothèque à jour : aucune fenêtre ne doit s’ouvrir, une notification confirme l’installation (et signale un brouillon non publié le cas échéant).
- Vérifier que le dialogue s’ouvre toujours quand il y a quelque chose à décider : bibliothèque en retard, hors ligne, harnais incompatible, modifications non enregistrées dans l’éditeur.

### Test utilisateur de l’étape 4

Vérifications effectuées : suite Rust complète (43 tests, dont 7 sur le référentiel et 2 sur l’édition ciblée du frontmatter), `pnpm check` sans erreur ni avertissement, 31 tests d’interface (dont les sélecteurs et la page d’administration, avec passerelle Tauri simulée), parcours CLI complet sur une bibliothèque temporaire. L’interface n’a pas été essayée dans l’application réelle.

- Ouvrir « Tags et catégories… » sur une bibliothèque sans `uber-skill.yaml` : vérifier l’explication, puis « Créer le référentiel à partir des valeurs utilisées » et le contenu du fichier créé.
- Ajouter une catégorie et un tag ; vérifier qu’ils apparaissent dans les sélecteurs de l’éditeur de métadonnées et du formulaire « Nouveau ».
- Renommer un tag utilisé par un skill et un agent : vérifier les deux fichiers, le référentiel, les filtres de la barre latérale, puis le diff dans « Publier… » (seules les lignes `tags`/`category` doivent changer lorsque le frontmatter est simple).
- Renommer vers un nom déjà existant : les deux valeurs doivent fusionner.
- Supprimer une valeur utilisée : le bouton reste désactivé sans choix ; tester « Remplacer par… » puis « Retirer des éléments ». Supprimer une valeur inutilisée.
- Ajouter à la main un tag inconnu dans un `SKILL.md` : vérifier le compteur dans la barre latérale, le marquage dans le détail, l’avertissement dans l’onglet Lint, puis « Ajouter au référentiel » et « Remplacer… ». Vérifier qu’un élément portant une valeur inconnue reste éditable et que la valeur est conservée tant qu’elle n’est pas retirée.
- Avec des modifications non enregistrées dans l’éditeur : vérifier que renommer, remplacer ou retirer une valeur utilisée est désactivé, mais qu’ajouter une valeur reste possible.
- Récupérer une mise à jour distante qui modifie `uber-skill.yaml` : vérifier que les sélecteurs suivent.

Limites de cette étape : pas de description ni de couleur par valeur ; les harnais (`hosts`) ne font pas partie du référentiel ; un frontmatter avec des tags en liste YAML ou des clés historiques au niveau racine est re-sérialisé en entier lors d’une modification.

### Test utilisateur de l’étape 2

Vérifications effectuées : suite Rust complète réussie (20 tests à ce stade), puis les 9 tests de publication réussis après ajout de deux cas supplémentaires ; `pnpm check` sans erreur ni avertissement ; parcours navigateur de publication avec passerelle Tauri simulée réussis. Les tests Rust exécutent de vrais commits et push vers des dépôts locaux temporaires, sans modifier le dépôt distant de l’utilisateur.

- Modifier et enregistrer deux fichiers, éventuellement l’un hors de l’application. Ouvrir « Publier… », ne cocher qu’un fichier, vérifier le diff, modifier le message puis lancer « Commit et push ». Vérifier sur le dépôt distant que seul le fichier choisi figure dans le nouveau commit et que l’autre reste un brouillon local.
- Si un fichier non coché est déjà préparé via `git add`, vérifier qu’il reste préparé et n’est pas inclus dans le commit.
- Tester l’ajout et la suppression d’un fichier ; pour un renommage, cocher l’ancien et le nouveau chemin.
- Modifier un fichier après ouverture de l’aperçu : la publication doit demander une actualisation avant de poursuivre.
- Si un push échoue après le commit (par exemple hors ligne), vérifier l’indication du commit local, puis réessayer « Envoyer les commits » après rétablissement de l’accès : aucun second commit ne doit être créé.
- Vérifier que les commits locaux déjà en attente sont annoncés avant envoi et que les modifications non enregistrées dans l’éditeur sont signalées.

Limites de cette étape : destination de suivi Git déjà configurée, conflits et opérations Git complexes traités à l’extérieur, pas de fetch automatique avant publication. Les fichiers sont sélectionnés en entier ; la sélection de fragments de diff n’est pas prévue.

### Test utilisateur de l’étape 5

Vérifications effectuées : suite Rust complète (44 tests, dont un sur les modèles et le refus d’écrasement), `pnpm check` sans erreur ni avertissement, 37 tests d’interface (dont le formulaire de création et l’ouverture en édition). L’interface n’a pas été essayée dans l’application réelle.

- Créer un skill avec « + Nouveau skill » : vérifier les sélecteurs de catégorie et de tags, l’ouverture directe dans l’onglet « Éditer », la structure du modèle (objectif, instructions, exemple d’utilisation) et la notification indiquant que le brouillon n’est pas publié.
- Faire de même pour un agent et vérifier que le modèle est différent (« Tu es … »).
- Essayer un nom déjà utilisé, puis le nom d’un dossier existant dans `skills/` qui n’est pas un skill : la création doit être refusée et rien ne doit être modifié.
- Vérifier que le brouillon apparaît dans « Publier… » comme un ajout, et qu’il peut être installé dans un projet avant publication (« Brouillon local non publié »).
- Vérifier l’onglet Lint du brouillon : aucun problème bloquant.

Limites de cette étape : les modèles sont intégrés à l’application et en français ; leur personnalisation depuis le dépôt reste une évolution possible.

### Test utilisateur de l’étape 6

Vérifications effectuées : suite Rust complète (49 tests, dont 5 sur le raffinement avec un faux exécutable `claude` : arguments, entrée, dossier d’exécution, erreurs, fichier modifié entre-temps), `pnpm check` sans erreur ni avertissement, 42 tests d’interface (dont le dialogue de raffinement), et un raffinement réel en ligne de commande sur un skill temporaire (environ 20 secondes, commentaire YAML et métadonnées préservés, fichier intact avant acceptation). L’interface n’a pas été essayée dans l’application réelle.

- Ouvrir un skill, cliquer sur « Raffiner… », saisir une consigne ou choisir une suggestion, puis « Proposer » : vérifier le message d’attente, puis le diff.
- Vérifier que le fichier n’est pas modifié tant que la proposition n’est pas acceptée, puis « Rejeter » : rien ne change.
- « Accepter » : le fichier est enregistré, l’éditeur affiche le nouveau texte, l’élément apparaît dans « Publier… » et, s’il est installé dans le projet courant, « Mettre à jour la copie du projet » est proposé.
- Vérifier que le nom, les tags, la catégorie et les harnais sont inchangés après acceptation, même avec une consigne demandant de les modifier (« renomme ce skill ») : la proposition doit signaler les clés rétablies.
- « Abandonner » pendant l’attente, puis relancer avec une autre consigne.
- Avec des modifications non enregistrées dans l’éditeur : « Raffiner… » doit être désactivé.
- Lancer l’application depuis le Finder (et non depuis un terminal) pour vérifier que `claude` est bien trouvé.
- Vérifier que « Raffiner… » n’apparaît pas pour un agent.

Limites de cette étape : skills uniquement, fichier `SKILL.md` uniquement ; l’appel ne peut pas être interrompu côté Claude (« Abandonner » ignore sa réponse) ; délai maximal de 5 minutes ; pas de choix du modèle dans l’application.

## Bibliothèque Git et installation

### Test utilisateur de l’étape 3

Vérifications effectuées : suite Rust complète (30 tests réussis), puis 4 tests d’installation réussis après ajout d’un cas sur les fichiers ignorés ; `pnpm check` sans erreur ni avertissement ; parcours navigateur de récupération, mise à jour puis installation, mode hors ligne, échec de mise à jour, installation groupée, agents et protection des métadonnées non enregistrées avec passerelle Tauri simulée. Les tests Rust utilisent de vrais dépôts distants temporaires locaux.

- Publier une modification depuis un autre clone, puis ouvrir « Récupérer… » : vérifier le nombre de commits distants, appliquer la mise à jour et vérifier le rechargement des skills et agents.
- Avec une bibliothèque en retard, demander une installation : tester séparément « Installer la version locale » et « Mettre à jour puis installer », puis vérifier le contenu copié.
- Répéter avec plusieurs éléments, un agent et une réinstallation depuis le tiroir « Installés ».
- Hors ligne, vérifier le message de fraîcheur non vérifiée et la possibilité d’installer explicitement la copie locale.
- Installer un brouillon ou un commit local non publié : vérifier l’état affiché avant installation et la mention « À l’installation » dans le tiroir, y compris après redémarrage.
- Avec des modifications locales qui recouvrent la mise à jour distante ou des historiques divergents, vérifier que la mise à jour s’arrête sans installation automatique ni perte des modifications. La résolution se fait dans un outil Git externe.
- Laisser du texte ou des métadonnées non enregistrés dans l’éditeur : vérifier que la mise à jour Git est désactivée jusqu’à leur enregistrement.

Précisions : la copie porte sur les fichiers enregistrés sur disque ; chaque lot est revérifié avant la première copie. L’état de la source à l’installation est ajouté au verrou de façon rétrocompatible. Les commandes CLI conservent pour le moment leur fonctionnement local existant ; ce parcours de vérification est celui de l’application de bureau. Le hash reste la référence de contenu ; l’ajout éventuel du commit d’origine reste distinct.

### Décisions validées

- Gérer la bibliothèque de skills et d’agents dans un dépôt Git, avec la possibilité de committer et pousser les modifications depuis l’application.
- Prendre en charge dès la première version les deux parcours d’ouverture : sélectionner un dépôt déjà cloné sur la machine, ou saisir une URL de dépôt distant et choisir un emplacement local pour le cloner depuis l’application puis l’ouvrir comme bibliothèque.
- Séparer explicitement « Enregistrer » et « Publier » : enregistrer sauvegarde les fichiers localement, sans commit ni push automatique ; publier affiche les changements et permet un commit/push explicite.
- Permettre de regrouper les modifications de plusieurs fichiers avant publication.
- Permettre d’installer dans un projet un brouillon local sans imposer sa publication préalable, notamment pour le tester.
- Afficher clairement que le contenu installé comporte des modifications locales non publiées.
- Vérifier les changements distants avant installation pour déterminer la fraîcheur de la bibliothèque locale.
- Si la bibliothèque locale est en retard, proposer « Mettre à jour puis installer » ou « Installer la version locale » ; le choix de la version locale reste explicite.
- Hors ligne, autoriser explicitement l’installation locale en indiquant que la fraîcheur n’a pas pu être vérifiée.
- Si la mise à jour rencontre un conflit, préserver les fichiers et les modifications locales, signaler le conflit et demander sa résolution dans un outil Git externe ; ne pas poursuivre automatiquement l’installation demandée avec mise à jour.

- Laisser la création d’un dépôt distant sur GitHub, GitLab ou un autre hébergeur en dehors de l’application.
- Proposer un bouton « Récupérer » pour mettre à jour la bibliothèque à tout moment, en complément de la vérification avant installation ; indiquer les changements locaux et distants en attente.
- Avant publication, présenter les différences et permettre de sélectionner les fichiers à inclure, y compris ceux modifiés hors de l’application ; permettre de conserver d’autres modifications à l’état de brouillon.
- Préremplir un message de commit modifiable, validé par l’utilisateur au moment de publier.
- Pour la première version, utiliser la branche actuellement ouverte et le dépôt distant configuré pour son suivi, sans interface de gestion des branches.
- Limiter les mises à jour intégrées à l’application à des avances rapides (fast-forward), sans fusion automatique, en préservant les modifications locales.
- Si les historiques local et distant divergent, demander une résolution dans un outil Git externe, même en l’absence de conflit de fichiers.

### Modalités techniques à préciser

- Mécanisme retenu : fetch de la branche distante configurée avant installation dans l’application et avant toute mise à jour en avance rapide.
- Traçabilité des installations : conserver le hash du contenu installé et étudier l’ajout du commit d’origine lorsqu’il correspond au contenu installé, sans présenter un brouillon comme une version publiée.

## Tags et catégories

### Décisions validées

- Définir les tags et catégories autorisés dans un fichier de configuration de la bibliothèque et proposer une interface d’administration pour les gérer.
- Adopter une approche tolérante pour les imports et les modifications effectuées hors de l’application : une valeur inconnue ne bloque ni l’import, ni l’affichage, ni l’édition d’un skill ou d’un agent.
- Signaler les valeurs inconnues et proposer leur mise en correspondance avec des valeurs autorisées ; conserver les valeurs existantes tant que l’utilisateur n’a pas choisi de correction.
- Versionner le fichier de configuration avec les skills et agents.
- Prévoir une catégorie facultative et plusieurs tags par élément.
- Partager le même référentiel entre skills et agents.
- Utiliser des sélecteurs de valeurs autorisées dans les champs de l’éditeur, tout en laissant visibles les valeurs inconnues déjà présentes.
- Intégrer l’administration dans une page « Bibliothèque » des réglages, avec le même circuit Enregistrer/Publier.
- Lors d’un renommage, mettre à jour les usages de la valeur ; lors de la suppression d’une valeur utilisée, demander son remplacement ou son retrait explicite des éléments concernés.

## Création à partir de templates

### Décisions validées

- Proposer un bouton « Nouveau skill / agent » avec saisie du nom, de la description et, facultativement, de la catégorie et des tags issus du référentiel.
- Créer le dossier contenant `SKILL.md` pour un skill, ou le fichier Markdown pour un agent, sans écraser un élément existant.
- Ouvrir immédiatement le brouillon créé dans l’éditeur ; la publication Git reste une action explicite de l’utilisateur.
- Fournir deux templates distincts, intégrés à l’application pour la première version, avec une structure courte : objectif, instructions et exemple d’utilisation.

### Évolution possible, hors première version

- Déplacer les templates dans le dépôt si le besoin de personnalisation apparaît, sous forme de modèles Markdown vierges ou d’un fichier de configuration. Le format reste à déterminer ; aucune gestion de templates personnalisés n’est prévue pour le moment.

## Raffinement assisté par Claude

### Décisions validées

- Proposer une action facultative « Raffiner » sur un skill, avec une consigne libre de l’utilisateur (clarifier les instructions, ajouter des exemples, réduire les ambiguïtés, etc.).
- Limiter la première version au contenu du fichier `SKILL.md` ; indiquer que les scripts et autres fichiers associés ne sont pas analysés.
- Produire une proposition et afficher les différences avec le contenu actuel, sans modifier le fichier avant acceptation explicite.
- Permettre d’accepter ou de rejeter la proposition ; un rejet conserve le contenu initial et une acceptation crée une modification locale, sans commit ni push automatique.
- Garder la fonction facultative et afficher une erreur claire si l’environnement Claude nécessaire n’est pas disponible.
- Ne pas présenter le raffinement ou le lint comme une validation du comportement du skill ; celui-ci doit être évalué par un essai réel.

### Modalités techniques à préciser

- Retenu : `claude -p --output-format json --tools "" --strict-mcp-config --no-session-persistence`, consigne et fichier transmis sur l’entrée standard, exécution dans un dossier temporaire vide, modèle par défaut du CLI (décision utilisateur), périmètre « corps + description » avec verrouillage du reste du frontmatter (décision utilisateur). Erreurs distinguées : commande introuvable, échec ou absence de connexion, délai dépassé, réponse qui n’est pas un `SKILL.md` valide, consigne vide.

### Évolution possible, hors première version

- Étendre le raffinement aux agents et, si nécessaire, aux fichiers associés aux skills.

## Évolutions à venir

Propositions du 21 septembre 2026, retenues par l’utilisateur pour être traitées au fur et à mesure. Aucune n’est spécifiée : chaque entrée liste l’objectif, ce qui existe déjà et les décisions à prendre avant de coder. L’ordre reflète le rapport bénéfice/effort estimé, pas un engagement.

### Préalables, avant d’ajouter des fonctionnalités

- **Passe UI/UX (étape 7)** : réalisée et validée par l’utilisateur.
- **Build de production** : réalisé le 21 septembre 2026, en attente du test utilisateur. `pnpm tauri build` produit `Uber Skill.app` (5,9 Mo) et un `.dmg` (2,9 Mo) pour Apple Silicon, signature ad hoc. Ajouts : icône propre à l’application, PATH complété au démarrage, aperçu Markdown assaini (DOMPurify) et politique de sécurité du contenu, métadonnées du bundle.
  - Vérifié : le build aboutit, l’application se lance et reste active, le frontend de production s’affiche sans aucune violation de la politique de sécurité dans un navigateur (seule erreur attendue : absence de la passerelle Tauri).
  - À vérifier par l’utilisateur, faute de pouvoir piloter la fenêtre native : lancer `Uber Skill.app` depuis le Finder (et non depuis un terminal), puis contrôler que la bibliothèque se charge, que « Récupérer… » et « Publier… » trouvent Git, que « Raffiner… » trouve `claude`, que « Ouvrir dans l’éditeur » fonctionne, que l’aperçu Markdown et ses liens s’affichent, et que l’icône apparaît dans le Dock.
  - Non traité : signature avec un certificat Apple et notarisation (nécessaires pour distribuer l’application à d’autres sans avertissement de macOS), mise à jour automatique, build Intel ou universel.

### 1. Vue « mes projets » : implémentée et validée par l’utilisateur

Décisions de l’utilisateur (21 septembre 2026) : liste explicite de projets suivis, alimentée à chaque installation et modifiable à la main ; scan de toutes les cibles qui contiennent un verrou ; un seul contrôle de fraîcheur pour un lot, avec installation directe quand il n’y a rien à décider ; vocabulaire « Projets », « copie en retard », « suivre un projet ».

Réalisé : `tracked_projects` dans la configuration (amorcée à partir des projets récents), `crates/core/src/projects.rs`, installation par lot (`prepare_batch` / `install_batch` : un seul fetch, tous les projets validés avant la première copie), onglet « Projets », section « Installé dans » du détail, notification après enregistrement, marqueur « ↻ n » dans la liste. La vérification visuelle a aussi conduit à corriger l’en-tête du détail, dont les boutons débordaient.

Vérifications effectuées : suite Rust complète (55 tests), `pnpm check`, 59 tests d’interface, et contrôle visuel du frontend compilé dans un navigateur avec une passerelle Tauri simulée et des données d’exemple. Non essayé dans l’application réelle.

À tester :
- Onglet « Projets » : vos projets récents doivent y figurer ; en installer un nouveau doit l’y ajouter.
- Modifier un skill installé dans deux projets : la notification doit annoncer les projets en retard, « Installé dans » doit les lister, et « Mettre à jour partout » doit les traiter en une fois.
- « Tout mettre à jour » sur un projet dont une copie a été modifiée à la main : cette copie ne doit pas être remplacée.
- Un projet avec deux cibles (par exemple `.claude` et `.cursor`) : les deux doivent apparaître, chacune nommée.
- Renommer ou déplacer un dossier de projet : il doit apparaître « introuvable », puis « Retirer de la liste ».
- Agir sur un projet qui n’est pas le projet courant (diff, retirer une copie), puis vérifier que le projet courant n’a pas changé.
- Passer sur « Projets » avec un fichier non enregistré dans l’éditeur, puis revenir : le texte doit être intact.

Reste possible plus tard : « Tout mettre à jour » pour l’ensemble des projets d’un coup, tri et recherche dans la liste des projets.

### 2. Installation globale : implémentée, en attente du test utilisateur

Décisions de l’utilisateur (22 septembre 2026) : harnais globaux limités à Claude Code et Codex, ceux réellement utilisés ; marqueur globe 🌐 dans les listes, coloré selon l’état de la copie globale ; mode d’emploi embarqué dans l’application.

Réalisé : le dossier personnel est un projet spécial « Global », toujours en tête de l’onglet Projets, scanné sur `~/.claude/skills`, `~/.claude/agents` et `~/.agents/skills` même sans verrou (les éléments installés à la main apparaissent « non suivis », avec « Lier » ou « Importer ») ; entrée « 🌐 Global (cette machine) » dans le sélecteur de projet, cibles limitées à Claude Code et Codex dans ce cas ; globe dans la liste, l’en-tête du détail et « Installé dans » ; libellé de la cible `.agents` renommé « Codex » ; guide embarqué derrière le bouton « ? » (`apps/desktop/src/lib/guide.md`).

Vérifications effectuées : suite Rust complète (56 tests), `pnpm check`, 61 tests d’interface, contrôle visuel du frontend compilé dans un navigateur. Non essayé dans l’application réelle.

À tester :
- Sélecteur de projet › « Global » : vos skills existants dans `~/.claude/skills` (dont le symlink `book-to-skill`) et `~/.agents/skills` doivent apparaître « non suivis » ; essayer « Lier » sur un élément présent dans la bibliothèque et « Importer » sur un autre.
- Installer un skill globalement, vérifier `~/.claude/skills/<id>` et le verrou, puis le globe vert dans la liste ; modifier le skill : le globe passe orange, « Installé dans » liste « Global ».
- Cible Codex en global : installer dans `~/.agents/skills` et vérifier que Codex le voit.
- « Retirer… » depuis Global : la confirmation doit dire « Global », pas le nom de votre dossier personnel.
- Bouton « ? » : lire le guide et signaler ce qui manque ou ce qui est faux.

### 3. Essayer un skill

- Objectif : vérifier qu’un skill se déclenche et fait ce qui est attendu, ce que ni le lint ni le raffinement ne garantissent. Action « Essayer » : lancer `claude -p` avec le skill chargé et une demande de test, afficher la réponse.
- Existant : lancement de `claude -p` (recherche de l’exécutable, délai, erreurs), installation d’un brouillon dans un dossier.
- À décider : où s’exécute l’essai (dossier temporaire avec le skill installé, ou projet choisi par l’utilisateur) ; quels outils autoriser et avec quel mode de permission, puisqu’un skill utile en a besoin contrairement au raffinement ; comment savoir si le skill s’est réellement déclenché (sortie `stream-json`) ; conservation de demandes de test par skill pour les rejouer ; coût et durée d’un essai. C’est la fonctionnalité la plus différenciante et la plus délicate : à spécifier soigneusement.

### 4. Corrections en un clic du lint

- Objectif : appliquer depuis l’onglet Lint la correction d’un lien cassé (la suggestion existe), d’un nom différent du dossier, d’un tag ou d’une catégorie inconnus (ajout au référentiel ou remplacement).
- Existant : règles de lint avec `rule`, suggestion de lien, opérations du référentiel, édition ciblée du frontmatter.
- À décider : forme de la correction renvoyée par le `core` (remplacement de texte localisé, ou action nommée) ; corrections proposées une par une ou groupées ; même mécanisme en ligne de commande (`lint --fix`). À coupler avec la traduction des messages de lint.

### 5. Historique d’un élément

- Objectif : onglet « Historique » avec les commits touchant l’élément, le diff de chacun, et « Revenir à cette version ». Rassure avant d’accepter un raffinement ou de récupérer une mise à jour distante.
- Existant : exécution de Git, affichage de diffs, états par élément.
- À décider : « Revenir à cette version » restaure les fichiers comme une modification locale à publier (recommandé) plutôt que de réécrire l’historique ; gestion des renommages et des éléments déplacés ; nombre de versions affichées ; historique d’un élément supprimé et restauration depuis Git en complément de la Corbeille.

### 6. Raffinement étendu

- Objectif : étendre le raffinement aux agents (déjà noté comme évolution possible), et proposer un raffinement de la seule `description`, avec deux ou trois variantes à comparer, puisqu’elle décide du déclenchement.
- Existant : `refine.rs`, verrouillage du frontmatter, dialogue de proposition.
- À décider : consigne système adaptée aux agents (clés `tools` et `model` verrouillées) ; présentation et choix entre plusieurs variantes ; inclure ou non les autres fichiers Markdown du skill dans ce qui est envoyé à Claude, et avec quelle limite de taille.

### 7. Import depuis une URL Git

- Objectif : importer un skill publié dans un dépôt (ou un sous-dossier de dépôt) en conservant sa provenance, et signaler quand l’amont a changé.
- Existant : clonage sécurisé, import d’un dossier local sans écrasement.
- À décider : où enregistrer la provenance (`metadata.source` dans le frontmatter, ou fichier à part) ; clonage temporaire puis copie, sans sous-module ; choix du sous-dossier quand le dépôt contient plusieurs skills ; comment comparer avec l’amont et proposer la mise à jour sans écraser des modifications locales ; attention portée au contenu importé (scripts) avant installation.

### 8. Dupliquer un élément

- Objectif : créer un skill ou un agent à partir d’un existant plutôt que du modèle.
- Existant : import avec nouvel identifiant, refus d’écraser.
- À décider : mise à jour automatique du `name` et du titre dans le fichier copié ; reprise ou non des tags et de la catégorie ; ouverture immédiate en édition comme pour une création.

### 9. Surveillance du disque

- Objectif : recharger la bibliothèque quand des fichiers changent hors de l’application, à la place du bouton « Recharger ».
- Existant : rechargement manuel, garde sur les modifications non enregistrées de l’éditeur.
- À décider : comportement quand le fichier ouvert dans l’éditeur interne change sur disque alors qu’il contient des modifications non enregistrées (ne jamais écraser, signaler) ; temporisation pour éviter les rechargements en rafale lors d’une opération Git ; surveiller aussi le projet courant pour l’état de dérive.

### 10. Recherche dans le contenu

- Objectif : chercher dans le corps des fichiers d’un skill, et pas seulement dans le nom, les tags et la description.
- Existant : recherche par jetons avec score, liste des fichiers texte d’un élément.
- À décider : recherche à la demande ou index en mémoire construit au scan ; fichiers inclus (Markdown seulement, ou tous les fichiers texte) ; affichage des extraits et ouverture du fichier trouvé à la bonne ligne ; poids du contenu par rapport au nom et aux tags dans le classement.

### Écarté pour le moment

- Gestion de branches, fusions et résolution de conflits dans l’application : les outils Git le font mieux, et la décision de s’en remettre à un outil externe a été validée.
- Éditeur enrichi (coloration, autocomplétion) : « Ouvrir dans l’éditeur » existe, et l’application n’a pas vocation à devenir un IDE.
- Partage public ou place de marché : prématuré tant que la bibliothèque est personnelle.
