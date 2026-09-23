/// Wording of lint findings and scan warnings. The core reports a code and its
/// arguments; the English `message` is only a fallback.

import type { Issue, ScanWarning } from "./api";
import { errorText } from "./errors";

export const SEVERITY_LABEL: Record<Issue["severity"], string> = {
  error: "Erreur",
  warning: "Avertissement",
  info: "Info",
};

const FINDINGS: Record<string, (args: string[]) => string> = {
  "file-missing": () => "Le fichier principal est absent (SKILL.md pour un skill, <nom>.md pour un agent).",
  "frontmatter-invalid": ([why]) => `Frontmatter illisible : ${why}`,
  "id-invalid": ([id]) => `« ${id} » n’est pas un identifiant valide : minuscules, chiffres et tirets, 64 caractères au plus.`,
  "name-missing": () => "Le champ « name » est absent du frontmatter.",
  "name-mismatch": ([name, id]) => `« name: ${name} » ne correspond pas au nom du dossier ou du fichier (« ${id} »).`,
  "description-missing": () => "La description est absente ou vide.",
  "description-too-long": ([n, max]) => `La description fait ${n} caractères (${max} au plus).`,
  "description-short": () => "La description est très courte : dites ce que fait l’élément et quand l’utiliser.",
  "tags-missing": () => "Aucun tag.",
  "category-missing": () => "Aucune catégorie.",
  "host-unknown": ([host]) => `Harnais inconnu : « ${host} » (connus : claude-code, codex, agents, amp, cursor, copilot).`,
  "body-empty": () => "Le fichier n’a pas de corps (instructions).",
  "body-long": ([lines]) => `Le corps fait ${lines} lignes : déplacez une partie du contenu dans des fichiers de référence.`,
  "path-missing": ([target]) => `Fichier mentionné introuvable : ${target}`,
  "link-missing": ([target, file, suggestion]) =>
    `Lien vers un fichier introuvable : ${target}` +
    (file && file !== "SKILL.md" ? ` (dans ${file})` : "") +
    (suggestion ? `. Ce fichier existe ici : ${suggestion}` : ""),
  "category-unknown": ([value]) => `La catégorie « ${value} » est absente du référentiel.`,
  "tag-unknown": ([value]) => `Le tag « ${value} » est absent du référentiel.`,
  "index-missing": () => "INDEX.md est absent de la bibliothèque : il sera généré à la prochaine modification.",
  "index-stale": () => "INDEX.md ne correspond plus aux éléments de la bibliothèque : il sera régénéré à la prochaine modification.",
};

export function issueText(issue: Issue): string {
  return FINDINGS[issue.code]?.(issue.args) ?? issue.message;
}

export function warningText(warning: ScanWarning): string {
  if (warning.duplicate_of) return `Identifiant déjà fourni par ${warning.duplicate_of} : cet élément est ignoré.`;
  return warning.error ? errorText(warning.error) : warning.message;
}
