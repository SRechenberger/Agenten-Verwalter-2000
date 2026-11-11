# Commit Conventions — verwalter

> Format: [Conventional Commits](https://www.conventionalcommits.org/). Projektentscheidungen in dieser Datei haben Vorrang.

## Format

```text
<type>(<scope>): <kurze Beschreibung im Imperativ>

<optionaler Body: warum, nicht was>
```

**Types:** `feat`, `fix`, `refactor`, `perf`, `docs`, `style`, `build`, `ci`, `chore`.

**Scopes:** die Feature- und Core-Namen aus der [Code-Map](../code-map.md): `projects`, `sessions`, `tldr`, `chat`, `changes`, `review`, `repositories`, `settings`, `attachments`, `skills`, `background`, `context`, `usage`, `account`, `mcp`, `mcp_access`, `notices`, `voice`, `artifacts`, `implementation`, `agent_types`, `remote`, `retro`, `terminal`, `agents`, `standalone`, `codex`, `git`, `worktrees`, `processes`, `db`, `ui`, `setup`, `docs`, `release` (Anhebung und Release-Pipeline, siehe [releases.md](releases.md)).

## Regeln

- Betreffzeile höchstens 72 Zeichen, kein Punkt am Ende
- Ein Commit = eine logische Änderung
- Doku-Änderungen, die zu einer Code-Änderung gehören (Code-Map, Glossar, Konventionen), kommen in denselben Commit
- Generierte Typen (`src/lib/bindings/`) werden zusammen mit der Rust-Änderung committet, die sie erzeugt

## Branches

- Ein Branch je logischer Änderung, abgezweigt von `dev`: `<präfix>/<thema>` in Kleinbuchstaben, ASCII, Wörter mit Bindestrich, Umlaute als `ae`, `oe`, `ue`
- Das Präfix ist der Commit-Type der Hauptänderung: `feature/` für `feat`, sonst `fix/`, `docs/`, `refactor/`, `chore/`
- Beispiele: `feature/unveraenderte-zeilen`, `docs/bau-voraussetzungen`
- Ausnahme ist `release/v<X.Y.Z>`, der Branch eines Releases ([releases.md](releases.md)): Er zweigt von `dev` ab, als Hotfix von `main`.

## Pull Requests

- Ziel ist `dev`. Nur ein Release-PR `release/v<X.Y.Z>` geht nach `main` ([releases.md](releases.md)).
- Der Titel ist eine Betreffzeile nach diesem Format plus `[MAJOR]`, `[MINOR]` oder `[PATCH]`, höchstens 72 Zeichen samt Annotation, zum Beispiel `feat(chat): Nachricht kürzen [MINOR]`. Ohne Annotation ist der Check `annotation` rot.
- Der Squash übernimmt den Titel als Betreff (GitHub hängt ` (#N)` an), und aus den Annotationen seit dem letzten Release folgt die nächste Version.
- Welche Annotation: ein Bruch ist `[MAJOR]`, eine neue Funktion `[MINOR]`, eine Fehlerbehebung, Doku oder CI `[PATCH]`.
- Merge-Methode je Ziel: nach `dev` **Squash**, nach `main` **Merge-Commit**, die Rückführung `release/v<X.Y.Z>` → `dev` **Merge-Commit**. Die Rulesets lassen nach `main` nur Merge-Commits zu, nach `dev` Squash und Merge-Commit.

## Critical Rules

1. **Keine Zugangsdaten, Tokens oder API-Schlüssel in Commits** — auch nicht in Beispielen oder Logs.
2. **Keine lokalen Datenbanken oder Worktrees committen** — `.gitignore` deckt sie ab; vor dem Commit `git status` lesen.
