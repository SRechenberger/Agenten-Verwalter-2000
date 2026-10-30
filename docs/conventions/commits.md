# Commit Conventions — verwalter

> Format: [Conventional Commits](https://www.conventionalcommits.org/). Projektentscheidungen in dieser Datei haben Vorrang.

## Format

```text
<type>(<scope>): <kurze Beschreibung im Imperativ>

<optionaler Body: warum, nicht was>
```

**Types:** `feat`, `fix`, `refactor`, `perf`, `docs`, `style`, `build`, `ci`, `chore`.

**Scopes:** die Feature- und Core-Namen aus der [Code-Map](../code-map.md): `projects`, `sessions`, `tldr`, `chat`, `changes`, `review`, `repositories`, `settings`, `attachments`, `skills`, `background`, `context`, `usage`, `account`, `mcp`, `notices`, `voice`, `artifacts`, `agents`, `standalone`, `codex`, `git`, `worktrees`, `processes`, `db`, `ui`, `setup`, `docs`, `release` (nur für den Versions-Commit, siehe [releases.md](releases.md)).

## Regeln

- Betreffzeile höchstens 72 Zeichen, kein Punkt am Ende
- Ein Commit = eine logische Änderung
- Doku-Änderungen, die zu einer Code-Änderung gehören (Code-Map, Glossar, Konventionen), kommen in denselben Commit
- Generierte Typen (`src/lib/bindings/`) werden zusammen mit der Rust-Änderung committet, die sie erzeugt

## Branches

- Ein Branch je logischer Änderung, abgezweigt von `main`: `<präfix>/<thema>` in Kleinbuchstaben, ASCII, Wörter mit Bindestrich, Umlaute als `ae`, `oe`, `ue`
- Das Präfix ist der Commit-Type der Hauptänderung: `feature/` für `feat`, sonst `fix/`, `docs/`, `refactor/`, `chore/`
- Beispiele: `feature/unveraenderte-zeilen`, `docs/bau-voraussetzungen`

## Critical Rules

1. **Keine Zugangsdaten, Tokens oder API-Schlüssel in Commits** — auch nicht in Beispielen oder Logs.
2. **Keine lokalen Datenbanken oder Worktrees committen** — `.gitignore` deckt sie ab; vor dem Commit `git status` lesen.
