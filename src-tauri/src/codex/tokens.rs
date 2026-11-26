//! Die Token-Zählung von Codex (`thread/tokenUsage/updated`) als Kontext-Angaben für den Verwalter.
use serde_json::{Value, json};

/// Name der einzigen Kategorie, die Codex hergibt: eine Aufschlüsselung wie bei Claude gibt es nicht.
const HISTORY_CATEGORY: &str = "Verlauf";
const KIND_USED: &str = "used";

/// Was der letzte Aufruf des Modells belegt hat und wie groß das Fenster ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenCount {
    pub input: u64,
    pub output: u64,
    pub window: Option<u64>,
}

impl TokenCount {
    /// `params` einer `thread/tokenUsage/updated`-Benachrichtigung; ohne `last` `None`.
    pub fn from_notification(params: &Value) -> Option<TokenCount> {
        let usage = params.get("tokenUsage")?;
        let last = usage.get("last")?;
        Some(TokenCount {
            input: last.get("inputTokens").and_then(Value::as_u64)?,
            output: last
                .get("outputTokens")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            window: usage.get("modelContextWindow").and_then(Value::as_u64),
        })
    }

    /// Antwort auf `get_context_usage` in der Form, die `agents/claude/stats.rs` liest.
    pub fn context_usage(&self, model: &str) -> Value {
        let used = self.input + self.output;
        json!({
            "model": model,
            "totalTokens": used,
            "maxTokens": self.window.unwrap_or(0),
            "categories": [{ "name": HISTORY_CATEGORY, "tokens": used, "kind": KIND_USED }],
        })
    }
}

/// Die letzte Zählung mit ihrer `message.id`. Codex schreibt sie in jede `assistant`-Zeile bis zur
/// nächsten Zählung (Denken, Werkzeug, Text, auch die ersten Zeilen des nächsten Zugs); mit derselben
/// id zählt der Verwalter einen Aufruf des Modells einmal (ADR 080).
#[derive(Debug, Default)]
pub struct LastCount {
    pub count: Option<TokenCount>,
    id: String,
}

impl LastCount {
    /// Eine neue id nur für eine neue Zählung: Meldet Codex dieselbe noch einmal, bleibt ihre id.
    /// Eine UUID statt eines Zählers, weil die Zeilen einen Neustart des Übersetzers überdauern.
    pub fn update(&mut self, count: TokenCount) {
        if self.count != Some(count) {
            self.id = format!("codex-{}", uuid::Uuid::new_v4());
        }
        self.count = Some(count);
    }

    pub fn apply(&self, line: &mut Value) {
        apply_to_assistant(line, self.count);
        if let Some(message) = line.get_mut("message").and_then(Value::as_object_mut)
            && message.contains_key("usage")
        {
            message.insert("id".to_owned(), json!(self.id));
        }
    }
}

/// Setzt die Token-Zählung in eine `assistant`-Zeile des Hauptagenten; ohne Zählung fällt das Feld
/// weg, damit der Verwalter den Balken nicht auf 0 setzt.
pub fn apply_to_assistant(line: &mut Value, count: Option<TokenCount>) {
    if line.get("type").and_then(Value::as_str) != Some("assistant") {
        return;
    }
    let Some(message) = line.get_mut("message").and_then(Value::as_object_mut) else {
        return;
    };
    match count {
        Some(count) => {
            message.insert(
                "usage".to_owned(),
                json!({ "input_tokens": count.input, "output_tokens": count.output }),
            );
        }
        None => {
            message.remove("usage");
        }
    }
}
