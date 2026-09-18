/// The core reports errors as a stable code plus data; the wording lives here.

export interface ErrorPayload {
  code: string;
  /// English, shown only when the code has no entry below.
  message: string;
  /// A path, an id or Git's own output. Never translated.
  details: string | null;
}

export type BlockReason =
  | "detached-head"
  | "no-commits"
  | "no-upstream"
  | "ambiguous-push-url"
  | "operation-in-progress"
  | "conflicts"
  | "diverged"
  | "submodule-change"
  | "unverified";

export const BLOCK_LABEL: Record<BlockReason, string> = {
  "detached-head": "HEAD est détachée. Ouvrez une branche dans votre outil Git.",
  "no-commits": "La branche locale ne contient aucun commit. Initialisez-la dans votre outil Git ou publiez un premier commit.",
  "no-upstream": "Cette branche n’a pas de branche distante de suivi utilisable. Configurez-la dans votre outil Git.",
  "ambiguous-push-url": "Le dépôt distant est absent ou comporte plusieurs destinations de push. Configurez une destination unique dans votre outil Git.",
  "operation-in-progress": "Une fusion, un rebase ou une autre opération Git est en cours. Terminez-la dans votre outil Git.",
  conflicts: "Des conflits Git restent à résoudre dans votre outil Git.",
  diverged: "Les historiques local et distant divergent. Résolvez cette divergence dans votre outil Git.",
  "submodule-change": "La publication de changements de sous-modules doit être effectuée dans votre outil Git.",
  unverified: "La fraîcheur distante n’a pas pu être vérifiée. Aucune mise à jour appliquée.",
};

const MESSAGES: Record<string, string> = {
  io: "Erreur de lecture ou d’écriture.",
  frontmatter: "Frontmatter invalide.",
  "not-found": "Élément introuvable dans la bibliothèque.",
  "already-exists": "Un élément porte déjà ce nom.",
  "invalid-id": "Identifiant invalide.",
  "no-library": "Aucune bibliothèque n’est configurée.",
  unsupported: "Cette cible ne gère pas ce type d’élément.",
  "not-a-directory": "Ce chemin n’est pas un dossier.",
  json: "JSON invalide.",
  yaml: "YAML invalide.",
  internal: "Erreur interne.",
  editor: "Impossible d’ouvrir l’éditeur.",
  "clone-not-saved": "Dépôt cloné, mais configuration non enregistrée. Vous pouvez ouvrir ce dépôt localement.",

  "git-unavailable": "Impossible de lancer Git. Vérifiez son installation.",
  "git-failed": "Échec de Git.",
  "git-clone-failed": "Le clonage n’a pas abouti. Vérifiez l’URL, le réseau et vos accès Git.",
  "git-commit-failed": "Le commit a échoué ; les fichiers sélectionnés restent préparés dans Git. Aucun push n’a été effectué.",
  "git-fast-forward-failed": "Mise à jour interrompue. Préservez vos modifications et résolvez la situation dans votre outil Git avant de réessayer.",
  "git-unexpected-output": "Réponse inattendue de Git.",
  "git-stale": "L’état a changé depuis la vérification. Actualisez avant de continuer.",

  "input.not-repository-root": "Choisissez la racine du dépôt Git.",
  "input.clone-url": "Indiquez une URL de dépôt Git valide.",
  "input.clone-name": "Indiquez un nom de dossier simple, sans séparateur de chemin.",
  "input.clone-destination": "Impossible de créer le dossier de destination. Choisissez un nouveau dossier.",
  "input.empty-selection": "Aucun élément sélectionné.",
  "input.unknown-selection": "La sélection contient un fichier absent de l’aperçu.",
  "input.empty-commit-message": "Le message de commit ne peut pas être vide.",
  "input.nothing-to-publish": "Sélectionnez des fichiers à publier. Aucun commit local n’est en attente.",
  "input.registry-value": "Valeur invalide : elle ne peut pas être vide, et un tag ne contient pas de virgule.",
  "input.registry-value-in-use": "Cette valeur est encore utilisée. Choisissez un remplacement ou son retrait des éléments concernés.",
};

function isPayload(e: unknown): e is ErrorPayload {
  return typeof e === "object" && e !== null && typeof (e as ErrorPayload).code === "string";
}

/// Text to show for anything a command or a promise can throw.
export function errorText(e: unknown): string {
  if (isPayload(e)) {
    const blocked = e.code.startsWith("blocked.") ? BLOCK_LABEL[e.code.slice("blocked.".length) as BlockReason] : undefined;
    const text = blocked ?? MESSAGES[e.code] ?? e.message;
    return e.details ? `${text}\n${e.details}` : text;
  }
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return JSON.stringify(e);
}
