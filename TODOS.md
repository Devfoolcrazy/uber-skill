# Fonctionnalités à spécifier

## Suivi de l’implémentation

Avancer étape par étape : chaque livraison doit être testée par l’utilisateur avant de commencer la suivante. Consigner les retours UI/UX ci-dessous et les traiter lors de la passe finale, une fois le fonctionnel posé.

1. **Ouverture d’une bibliothèque Git locale et clonage depuis une URL : implémentés et validés par l’utilisateur.** Interface « Ouvrir / Cloner… » commune aux skills et agents, validation de la racine Git, destination de clonage nouvelle uniquement, réinitialisation de l’ancien chemin d’agents personnalisé, choix conservé au prochain lancement. Les bibliothèques préexistantes sans Git restent lisibles au démarrage.
2. **Publication Git : implémentée et validée par l’utilisateur.** Aperçu, sélection par fichier, message prérempli modifiable, commit/push explicite et nouvel essai du push sans commit supplémentaire.
3. **Récupération Git et vérification de fraîcheur avant installation : implémentées et validées par l’utilisateur.** Fetch de la branche distante configurée, compteur de commits locaux/distants, mise à jour en avance rapide et choix explicite d’une installation locale. Contrôle commun aux installations unitaires, groupées, agents et réinstallations depuis le tiroir.
4. **Référentiel de tags/catégories et administration : implémentés et validés par l’utilisateur.** Fichier `uber-skill.yaml` versionné à la racine, commun aux skills et agents, tolérant aux valeurs inconnues ; page « Tags et catégories » (ajout, renommage partout, suppression avec remplacement ou retrait explicite, mise en correspondance des valeurs inconnues) ; sélecteurs dans l’éditeur et le formulaire de création ; avertissements de lint ; commandes CLI `registry`.
5. **Création de skills/agents avec templates intégrés : implémentée et validée par l’utilisateur.** Modèles distincts dans `crates/core/src/templates/`, refus d’écraser quoi que ce soit, ouverture immédiate du brouillon dans l’éditeur.
6. **Raffinement assisté du `SKILL.md` : implémenté, en attente du test utilisateur.** `claude -p` sans outil ni session, modèle par défaut du CLI ; la proposition peut modifier le corps et la description, le reste du frontmatter est verrouillé ; diff, puis acceptation ou rejet explicites.
7. Passe UI/UX : après validation du fonctionnel.

### Test utilisateur de l’étape 1

Vérifications effectuées : `cargo test` (13 tests réussis, dont 3 tests Git sur des dépôts temporaires), `pnpm check` (aucune erreur ni avertissement), parcours d’interface automatisés avec passerelle Tauri simulée (ouverture, erreurs, clonage, nouvelle tentative, fermeture). Le clonage distant avec les identifiants réels reste à vérifier par l’utilisateur.

- Ouvrir la racine d’un dépôt local contenant `skills/` et `agents/`, puis vérifier les deux onglets et la persistance du choix après redémarrage.
- Cloner un dépôt accessible dans un nouveau dossier, puis vérifier le chargement des contenus.
- Essayer un dossier sans Git, une destination de clonage existante et une URL inaccessible : vérifier l’erreur et la conservation de la bibliothèque précédente.
- Si un fichier a des modifications non enregistrées dans l’éditeur, vérifier la confirmation avant changement de bibliothèque.

### Retours UI/UX à traiter lors de la passe finale

- Retour utilisateur (test de l’étape 4) : confusion entre la publication Git et la mise à jour de la copie du projet ; après « Publier… », l’élément restait « Bibliothèque plus récente » et le contrôle d’installation ne proposait que « Installer la version locale », « Mettre à jour puis installer » restant grisé. Traité : libellés distinguant les deux synchronisations, une seule action « Installer dans le projet » quand la bibliothèque est à jour, mise à jour de la bibliothèque proposée seulement quand elle est en retard. Traité aussi, à la demande de l’utilisateur : après l’enregistrement d’un élément installé dans le projet courant, le détail propose directement « Mettre à jour la copie du projet » et la notification le signale. À revoir lors de la passe finale : éviter le dialogue quand il n’y a rien à décider.
- Relevé pendant l’étape 4 : les messages de lint et les avertissements de scan sont encore rédigés en anglais dans le `core` et affichés tels quels dans l’application. Les traduire côté application à partir de `rule`, selon la convention des codes d’erreur.

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
