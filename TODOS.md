# Fonctionnalités à spécifier

## Suivi de l’implémentation

Avancer étape par étape : chaque livraison doit être testée par l’utilisateur avant de commencer la suivante. Consigner les retours UI/UX ci-dessous et les traiter lors de la passe finale, une fois le fonctionnel posé.

1. **Ouverture d’une bibliothèque Git locale et clonage depuis une URL : implémentés et validés par l’utilisateur.** Interface « Ouvrir / Cloner… » commune aux skills et agents, validation de la racine Git, destination de clonage nouvelle uniquement, réinitialisation de l’ancien chemin d’agents personnalisé, choix conservé au prochain lancement. Les bibliothèques préexistantes sans Git restent lisibles au démarrage.
2. **Publication Git : implémentée et validée par l’utilisateur.** Aperçu, sélection par fichier, message prérempli modifiable, commit/push explicite et nouvel essai du push sans commit supplémentaire.
3. **Récupération Git et vérification de fraîcheur avant installation : implémentées dans l’application, en attente du test utilisateur.** Fetch de la branche distante configurée, compteur de commits locaux/distants, mise à jour en avance rapide et choix explicite d’une installation locale. Contrôle commun aux installations unitaires, groupées, agents et réinstallations depuis le tiroir.
4. Référentiel de tags/catégories et administration : à implémenter.
5. Création de skills/agents avec templates intégrés : à compléter selon les décisions ci-dessous.
6. Raffinement assisté du `SKILL.md` : à implémenter.
7. Passe UI/UX : après validation du fonctionnel.

### Test utilisateur de l’étape 1

Vérifications effectuées : `cargo test` (13 tests réussis, dont 3 tests Git sur des dépôts temporaires), `pnpm check` (aucune erreur ni avertissement), parcours d’interface automatisés avec passerelle Tauri simulée (ouverture, erreurs, clonage, nouvelle tentative, fermeture). Le clonage distant avec les identifiants réels reste à vérifier par l’utilisateur.

- Ouvrir la racine d’un dépôt local contenant `skills/` et `agents/`, puis vérifier les deux onglets et la persistance du choix après redémarrage.
- Cloner un dépôt accessible dans un nouveau dossier, puis vérifier le chargement des contenus.
- Essayer un dossier sans Git, une destination de clonage existante et une URL inaccessible : vérifier l’erreur et la conservation de la bibliothèque précédente.
- Si un fichier a des modifications non enregistrées dans l’éditeur, vérifier la confirmation avant changement de bibliothèque.

### Retours UI/UX à traiter lors de la passe finale

Aucun retour consigné pour le moment.

### Test utilisateur de l’étape 2

Vérifications effectuées : suite Rust complète réussie (20 tests à ce stade), puis les 9 tests de publication réussis après ajout de deux cas supplémentaires ; `pnpm check` sans erreur ni avertissement ; parcours navigateur de publication avec passerelle Tauri simulée réussis. Les tests Rust exécutent de vrais commits et push vers des dépôts locaux temporaires, sans modifier le dépôt distant de l’utilisateur.

- Modifier et enregistrer deux fichiers, éventuellement l’un hors de l’application. Ouvrir « Publier… », ne cocher qu’un fichier, vérifier le diff, modifier le message puis lancer « Commit et push ». Vérifier sur le dépôt distant que seul le fichier choisi figure dans le nouveau commit et que l’autre reste un brouillon local.
- Si un fichier non coché est déjà préparé via `git add`, vérifier qu’il reste préparé et n’est pas inclus dans le commit.
- Tester l’ajout et la suppression d’un fichier ; pour un renommage, cocher l’ancien et le nouveau chemin.
- Modifier un fichier après ouverture de l’aperçu : la publication doit demander une actualisation avant de poursuivre.
- Si un push échoue après le commit (par exemple hors ligne), vérifier l’indication du commit local, puis réessayer « Envoyer les commits » après rétablissement de l’accès : aucun second commit ne doit être créé.
- Vérifier que les commits locaux déjà en attente sont annoncés avant envoi et que les modifications non enregistrées dans l’éditeur sont signalées.

Limites de cette étape : destination de suivi Git déjà configurée, conflits et opérations Git complexes traités à l’extérieur, pas de fetch automatique avant publication. Les fichiers sont sélectionnés en entier ; la sélection de fragments de diff n’est pas prévue.

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

- Envisager `claude -p` en mode headless ; préciser les prérequis, les paramètres d’exécution et la gestion des erreurs lors de la spécification technique.

### Évolution possible, hors première version

- Étendre le raffinement aux agents et, si nécessaire, aux fichiers associés aux skills.
