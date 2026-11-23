//! Prompt-Vorlagen (ADR 075): Prüfung mit ihren Meldungen, Speicher, Ereignis und Migration 35.
use std::cell::Cell;
use std::path::Path;

use super::model::{
    PromptTemplate, PromptTemplateError, PromptTemplateField, PromptTemplateInput,
    PromptTemplateSaved,
};
use super::{create, delete, list, update, validate};
use crate::db::Database;

fn input(name: &str, text: &str, name_template: &str) -> PromptTemplateInput {
    PromptTemplateInput {
        name: name.to_owned(),
        text: text.to_owned(),
        name_template: name_template.to_owned(),
    }
}

fn stored(id: &str, name: &str) -> PromptTemplate {
    PromptTemplate {
        id: id.to_owned(),
        name: name.to_owned(),
        text: "Text".to_owned(),
        name_template: String::new(),
    }
}

fn errors_of(input: &PromptTemplateInput, existing: &[PromptTemplate]) -> Vec<PromptTemplateError> {
    validate(input, existing, None).expect_err("abgelehnt")
}

fn error(field: PromptTemplateField, message: &str) -> PromptTemplateError {
    PromptTemplateError {
        field,
        message: message.to_owned(),
    }
}

fn database() -> Database {
    Database::open(Path::new(":memory:")).expect("Datenbank im Speicher")
}

/// Legt an und zählt, wie oft das Ereignis ginge.
fn saved_with_count(
    database: &Database,
    input: &PromptTemplateInput,
    events: &Cell<u32>,
) -> PromptTemplateSaved {
    create(database, input, |_| events.set(events.get() + 1)).expect("Datenbank")
}

#[test]
fn a_template_without_name_is_rejected() {
    assert_eq!(
        errors_of(&input("  ", "Text", ""), &[]),
        vec![error(PromptTemplateField::Name, "Der Name fehlt.")]
    );
}

#[test]
fn a_name_over_40_chars_is_rejected() {
    assert!(validate(&input(&"ä".repeat(40), "Text", ""), &[], None).is_ok());
    assert_eq!(
        errors_of(&input(&"ä".repeat(41), "Text", ""), &[]),
        vec![error(
            PromptTemplateField::Name,
            "Der Name ist länger als 40 Zeichen."
        )]
    );
}

#[test]
fn a_name_taken_by_another_template_is_rejected_ignoring_case() {
    assert_eq!(
        errors_of(&input(" bugfix ", "Text", ""), &[stored("a", "Bugfix")]),
        vec![error(
            PromptTemplateField::Name,
            "Der Name „bugfix“ gehört schon einer anderen Vorlage."
        )]
    );
}

#[test]
fn a_template_keeps_its_own_name_on_update_in_another_spelling() {
    assert!(
        validate(
            &input("BUGFIX", "Text", ""),
            &[stored("a", "Bugfix")],
            Some("a")
        )
        .is_ok()
    );
}

#[test]
fn an_empty_or_blank_text_is_rejected() {
    for text in ["", " \n\t"] {
        assert_eq!(
            errors_of(&input("Bugfix", text, ""), &[]),
            vec![error(PromptTemplateField::Text, "Der Vorlagentext fehlt.")]
        );
    }
}

#[test]
fn a_text_over_16000_utf16_units_is_rejected() {
    // 8 000 Emoji sind 16 000 UTF-16-Einheiten, aber nur 8 000 Zeichen.
    assert!(validate(&input("Bugfix", &"😀".repeat(8_000), ""), &[], None).is_ok());
    assert_eq!(
        errors_of(
            &input("Bugfix", &format!("{}a", "😀".repeat(8_000)), ""),
            &[]
        ),
        vec![error(
            PromptTemplateField::Text,
            "Der Vorlagentext ist länger als 16 000 Zeichen."
        )]
    );
}

#[test]
fn a_name_template_over_200_chars_is_rejected() {
    assert!(validate(&input("Bugfix", "Text", &"n".repeat(200)), &[], None).is_ok());
    assert_eq!(
        errors_of(&input("Bugfix", "Text", &"n".repeat(201)), &[]),
        vec![error(
            PromptTemplateField::NameTemplate,
            "Die Namensvorlage ist länger als 200 Zeichen."
        )]
    );
}

#[test]
fn a_syntax_error_in_the_text_is_rejected_at_the_text_with_line_and_column() {
    assert_eq!(
        errors_of(&input("Bugfix", "Zeile\n  {{#Mit Test}}x", ""), &[]),
        vec![error(
            PromptTemplateField::Text,
            "Zeile 2, Spalte 3: Der Abschnitt „Mit Test“ ist nicht geschlossen."
        )]
    );
}

#[test]
fn a_syntax_error_in_the_name_template_is_rejected_at_the_name_template() {
    assert_eq!(
        errors_of(&input("Bugfix", "Text", "Bugfix: {{#a}}"), &[]),
        vec![error(
            PromptTemplateField::NameTemplate,
            "Zeile 1, Spalte 9: Der Abschnitt „a“ ist nicht geschlossen."
        )]
    );
}

#[test]
fn a_syntax_error_in_the_name_template_names_line_and_column_as_typed_like_the_editor() {
    // Der Editor prüft `"\n  {{#x}}"` wie eingegeben: Zeile 2, Spalte 3. Getrimmt wäre es Zeile 1.
    assert_eq!(
        errors_of(&input("Bugfix", "Text", "\n  {{#x}}"), &[]),
        vec![error(
            PromptTemplateField::NameTemplate,
            "Zeile 2, Spalte 3: Der Abschnitt „x“ ist nicht geschlossen."
        )]
    );
    let valid = validate(&input("Bugfix", "Text", "\n {{x}} "), &[], None).expect("gültig");
    assert_eq!(valid.name_template, "{{x}}", "gespeichert wird getrimmt");
}

#[test]
fn errors_of_several_fields_come_together_in_field_order() {
    let errors = errors_of(&input("", "", &"n".repeat(201)), &[]);
    let fields: Vec<PromptTemplateField> = errors
        .iter()
        .map(|error: &PromptTemplateError| error.field)
        .collect();
    assert_eq!(
        fields,
        vec![
            PromptTemplateField::Name,
            PromptTemplateField::Text,
            PromptTemplateField::NameTemplate
        ]
    );
}

#[test]
fn a_valid_template_comes_back_with_name_and_name_template_trimmed_and_the_text_untouched() {
    let valid = validate(
        &input("  Bugfix ", "  {{x}}\n", " Bugfix: {{x}} "),
        &[],
        None,
    )
    .expect("gültig");
    assert_eq!(valid, input("Bugfix", "  {{x}}\n", "Bugfix: {{x}}"));
}

#[test]
fn templates_are_listed_in_the_order_they_were_created() {
    let database = database();
    for name in ["Zweitens", "Alpha", "Mitte"] {
        create(&database, &input(name, "Text", ""), |_| {}).expect("Datenbank");
    }
    let names: Vec<String> = list(&database)
        .expect("Liste")
        .into_iter()
        .map(|prompt_template: PromptTemplate| prompt_template.name)
        .collect();
    assert_eq!(names, vec!["Zweitens", "Alpha", "Mitte"]);
}

#[test]
fn update_keeps_id_and_place_and_stores_the_new_values() {
    let database = database();
    let first = create(&database, &input("Eins", "Text", ""), |_| {})
        .expect("Datenbank")
        .prompt_template
        .expect("gespeichert");
    create(&database, &input("Zwei", "Text", ""), |_| {}).expect("Datenbank");
    let saved = update(
        &database,
        &first.id,
        &input(" Umbenannt ", "Neu {{x}}", "N: {{x}}"),
        |_| {},
    )
    .expect("Datenbank");
    let expected = PromptTemplate {
        id: first.id.clone(),
        name: "Umbenannt".to_owned(),
        text: "Neu {{x}}".to_owned(),
        name_template: "N: {{x}}".to_owned(),
    };
    assert_eq!(saved.prompt_template, Some(expected.clone()));
    assert_eq!(list(&database).expect("Liste")[0], expected);
}

#[test]
fn updating_or_deleting_an_unknown_template_is_an_error() {
    let database = database();
    assert!(update(&database, "fehlt", &input("A", "Text", ""), |_| {}).is_err());
    assert!(delete(&database, "fehlt", |_| {}).is_err());
}

#[test]
fn delete_removes_the_template_and_keeps_the_order_of_the_rest() {
    let database = database();
    let ids: Vec<String> = ["A", "B", "C"]
        .iter()
        .map(|name: &&str| {
            create(&database, &input(name, "Text", ""), |_| {})
                .expect("Datenbank")
                .prompt_template
                .expect("gespeichert")
                .id
        })
        .collect();
    delete(&database, &ids[1], |_| {}).expect("gelöscht");
    let names: Vec<String> = list(&database)
        .expect("Liste")
        .into_iter()
        .map(|prompt_template: PromptTemplate| prompt_template.name)
        .collect();
    assert_eq!(names, vec!["A", "C"]);
}

#[test]
fn the_change_event_goes_out_only_after_a_successful_save_or_delete() {
    let database = database();
    let events = Cell::new(0);
    let refused = saved_with_count(&database, &input("", "{{#a}}", ""), &events);
    assert!(refused.prompt_template.is_none() && !refused.errors.is_empty());
    assert_eq!(events.get(), 0, "abgelehnt: kein Ereignis");
    assert!(
        list(&database).expect("Liste").is_empty(),
        "nichts gespeichert"
    );

    let first = saved_with_count(&database, &input("A", "Text", ""), &events)
        .prompt_template
        .expect("gespeichert");
    assert_eq!(events.get(), 1);
    let duplicate = saved_with_count(&database, &input("a", "Text", ""), &events);
    assert!(duplicate.prompt_template.is_none());
    assert_eq!(events.get(), 1, "doppelter Name: kein Ereignis");

    let mut payload: Vec<PromptTemplate> = Vec::new();
    update(&database, &first.id, &input("A", "{{/x}}", ""), |_| {
        events.set(events.get() + 1);
    })
    .expect("Datenbank");
    assert_eq!(events.get(), 1, "Syntaxfehler beim Ändern: kein Ereignis");
    update(&database, &first.id, &input("B", "Text", ""), |all| {
        payload = all;
    })
    .expect("Datenbank");
    assert_eq!(payload.len(), 1, "die ganze Liste");
    assert_eq!(payload[0].name, "B");

    assert!(
        delete(&database, "fehlt", |_| events.set(events.get() + 1)).is_err() && events.get() == 1,
        "unbekannt: kein Ereignis"
    );
    delete(&database, &first.id, |all| {
        assert!(all.is_empty());
        events.set(events.get() + 1);
    })
    .expect("gelöscht");
    assert_eq!(events.get(), 2);
}

#[test]
fn migration_35_adds_the_empty_template_table_to_a_database_of_version_34() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("Datenbank");
    for script in crate::db::migrations::first(34) {
        connection.execute_batch(script).expect("alte Migration");
    }
    connection
        .execute_batch(
            "PRAGMA user_version = 34; \
             INSERT INTO projects (id, name, created_at) VALUES ('p', 'V', 1.0);",
        )
        .expect("Bestand");
    crate::db::migrations::run(&mut connection).expect("wandert");
    let version: u32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("Version");
    assert_eq!(version, 35);
    assert!(
        crate::db::prompt_templates::list(&connection)
            .expect("Tabelle da")
            .is_empty(),
        "keine mitgelieferten Vorlagen"
    );
    let projects: u32 = connection
        .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
        .expect("Vorhaben");
    assert_eq!(projects, 1, "der Bestand bleibt");
}

// Vorschläge per MCP (ADR 075, „Per MCP“), nach dem Muster der Agententypen (ADR 063): Schema, Eingabe,
// Prüfung, Ergebnis, Veraltet und Klasse des Befehls. Alles über freie Funktionen: Ein `Box<dyn ToolSet>`
// der echten Sätze nähme Tauri ins Testprogramm.
mod per_mcp {
    use std::cell::Cell;

    use serde_json::{Value, json};

    use super::super::model::{
        PromptTemplate, PromptTemplateError, PromptTemplateField, PromptTemplateInput,
        PromptTemplateSaved,
    };
    use super::super::{create, delete, list, tools, update, update_if_unchanged};
    use super::{database, error, input};
    use crate::agents::event::{QuestionAnswer, ToolRequest};
    use crate::error::CommandError;
    use crate::mcp_access::tools::ToolSpec;
    use crate::sessions::registry::tool_answer_label;

    fn tool(name: &str) -> ToolSpec {
        tools::specs()
            .into_iter()
            .find(|spec: &ToolSpec| spec.name == name)
            .expect("Werkzeug vorhanden")
    }

    fn bugfix() -> PromptTemplate {
        PromptTemplate {
            id: "id-bugfix".to_owned(),
            name: "Bugfix".to_owned(),
            text: "Behebe {{Fehler}}.\n{{#Mit Test}}\nMit Test.\n{{/Mit Test}}".to_owned(),
            name_template: "Bugfix: {{Fehler}}".to_owned(),
        }
    }

    #[test]
    fn the_three_tools_have_their_schemas_without_additional_properties() {
        let names: Vec<&str> = tools::specs()
            .iter()
            .map(|spec: &ToolSpec| spec.name)
            .collect();
        assert_eq!(
            names,
            [
                "prompt_template_list",
                "prompt_template_propose_create",
                "prompt_template_propose_update"
            ]
        );
        let list_tool = tool("prompt_template_list");
        let create_tool = tool("prompt_template_propose_create");
        let update_tool = tool("prompt_template_propose_update");
        assert_eq!(list_tool.input_schema["properties"], json!({}));
        assert!(list_tool.input_schema.get("required").is_none());
        assert_eq!(
            create_tool.input_schema["required"],
            json!(["name", "text"])
        );
        assert_eq!(update_tool.input_schema["required"], json!(["target"]));
        assert!(
            create_tool.input_schema["properties"]
                .get("target")
                .is_none()
        );
        for field in ["name", "text", "nameTemplate"] {
            for spec in [&create_tool, &update_tool] {
                assert_eq!(
                    spec.input_schema["properties"][field]["type"],
                    json!("string"),
                    "{field} in {}",
                    spec.name
                );
            }
        }
        for spec in [&list_tool, &create_tool, &update_tool] {
            assert_eq!(
                spec.input_schema["additionalProperties"],
                json!(false),
                "{}: ein Tippfehler im Feldnamen scheitert am Schema",
                spec.name
            );
        }
    }

    #[test]
    fn the_proposal_descriptions_name_the_syntax() {
        for spec in [
            tool("prompt_template_propose_create"),
            tool("prompt_template_propose_update"),
        ] {
            for part in [
                "{{Name}}",
                "{{#Name}}…{{/Name}}",
                "{{^Name}}",
                "{{! … }}",
                "geschachtelt",
            ] {
                assert!(
                    spec.description.contains(part),
                    "{}: {part} fehlt",
                    spec.name
                );
            }
        }
    }

    #[test]
    fn only_the_list_is_read_only_and_allowed_in_side_questions() {
        assert!(tools::is_read_only("prompt_template_list"));
        assert!(tools::side_may_call("prompt_template_list"));
        for writing in [
            "prompt_template_propose_create",
            "prompt_template_propose_update",
        ] {
            assert!(!tools::is_read_only(writing), "{writing} ist nicht lesend");
            assert!(
                !tools::side_may_call(writing),
                "{writing} bleibt der Nebenfrage verboten"
            );
        }
    }

    #[test]
    fn the_list_returns_every_template_whole_in_creation_order_without_id() {
        let templates = vec![bugfix(), super::stored("id-2", "Refactoring")];
        let expected = json!({ "promptTemplates": [
            {
                "name": "Bugfix",
                "text": "Behebe {{Fehler}}.\n{{#Mit Test}}\nMit Test.\n{{/Mit Test}}",
                "nameTemplate": "Bugfix: {{Fehler}}",
            },
            { "name": "Refactoring", "text": "Text", "nameTemplate": "" },
        ] });
        assert_eq!(
            tools::list_answer(&json!({}), &templates),
            Ok(expected.clone())
        );
        assert_eq!(tools::list_answer(&Value::Null, &templates), Ok(expected));
        assert!(
            tools::list_answer(&json!({ "name": "Bugfix" }), &templates).is_err(),
            "unbekannte Argumente werden abgelehnt"
        );
    }

    #[test]
    fn propose_create_needs_name_and_text_and_leaves_a_missing_name_template_empty() {
        assert_eq!(
            tools::create_input(&json!({ "name": "Refactoring", "text": "Baue {{Modul}} um." })),
            Ok(input("Refactoring", "Baue {{Modul}} um.", ""))
        );
        assert_eq!(
            tools::create_input(&json!({ "name": "R", "text": "T", "nameTemplate": "R: {{x}}" })),
            Ok(input("R", "T", "R: {{x}}"))
        );
        assert!(
            tools::create_input(&json!({ "name": "Refactoring" }))
                .is_err_and(|error: String| error.contains("text"))
        );
        assert!(tools::create_input(&json!({ "name": "R", "text": "T", "target": "X" })).is_err());
        assert!(
            tools::create_input(&json!({ "name": "R", "text": "T", "name_template": "x" }))
                .is_err(),
            "unbekannte Felder werden abgelehnt"
        );
    }

    #[test]
    fn propose_update_finds_the_target_ignoring_case_and_keeps_missing_fields() {
        let templates = vec![bugfix()];
        let (base, kept) =
            tools::update_input(&json!({ "target": " BUGFIX ", "text": "Neu." }), &templates)
                .expect("Ziel gefunden");
        assert_eq!(base, templates[0]);
        assert_eq!(
            kept,
            PromptTemplateInput {
                text: "Neu.".to_owned(),
                ..PromptTemplateInput::from(&templates[0])
            },
            "ein fehlendes Feld behält den gespeicherten Wert"
        );
        let (_, emptied) = tools::update_input(
            &json!({ "target": "Bugfix", "nameTemplate": "" }),
            &templates,
        )
        .expect("Ziel gefunden");
        assert_eq!(
            emptied.name_template, "",
            "ein genanntes leeres Feld ersetzt"
        );
        assert_eq!(
            tools::update_input(&json!({ "target": "Feature" }), &templates).err(),
            Some("Keine Vorlage „Feature“.".to_owned())
        );
        assert!(tools::update_input(&json!({ "name": "X" }), &templates).is_err());
    }

    #[test]
    fn an_invalid_proposal_becomes_one_line_per_error_with_the_syntax_position_without_a_box() {
        let templates = vec![bugfix()];
        assert_eq!(
            tools::checked(
                &input("bugfix", "Zeile\n  {{#Mit Test}}x", ""),
                &templates,
                None
            ),
            Err(
                "name: Der Name „bugfix“ gehört schon einer anderen Vorlage.\n\
                 text: Zeile 2, Spalte 3: Der Abschnitt „Mit Test“ ist nicht geschlossen."
                    .to_owned()
            )
        );
        assert_eq!(
            tools::checked(&input("", &"x".repeat(16_001), ""), &templates, None),
            Err("name: Der Name fehlt.\n\
                 text: Der Vorlagentext ist länger als 16 000 Zeichen."
                .to_owned())
        );
        assert_eq!(
            tools::checked(&input("R", "T", "R: {{#a}}"), &templates, None),
            Err(
                "nameTemplate: Zeile 1, Spalte 4: Der Abschnitt „a“ ist nicht geschlossen."
                    .to_owned()
            ),
            "das Feld heißt wie PromptTemplateField"
        );
        // Bei einer Änderung kollidiert die Vorlage nicht mit sich selbst; die Eingabe kommt getrimmt.
        let (base, own) = tools::update_input(
            &json!({ "target": "Bugfix", "name": " Bugfix ", "nameTemplate": " B: {{Fehler}} " }),
            &templates,
        )
        .expect("Ziel gefunden");
        assert_eq!(
            tools::checked(&own, &templates, Some(&base.id)),
            Ok(input("Bugfix", &base.text, "B: {{Fehler}}"))
        );
    }

    #[test]
    fn the_result_marks_edited_only_when_the_saved_template_differs_and_answer_lines_follow() {
        let saved = bugfix();
        let proposal = PromptTemplateInput::from(&saved);
        assert_eq!(
            tools::accepted_result(&saved, &proposal),
            json!({ "decision": "accepted", "edited": false, "promptTemplate": {
                "id": "id-bugfix", "name": "Bugfix", "text": saved.text, "nameTemplate": "Bugfix: {{Fehler}}",
            } })
        );
        let other = PromptTemplateInput {
            text: "Vorgeschlagen.".to_owned(),
            ..proposal
        };
        assert_eq!(
            tools::accepted_result(&saved, &other)["edited"],
            json!(true)
        );
        assert_eq!(
            tools::rejected_result("  zu breit  "),
            json!({ "decision": "rejected", "reason": "zu breit" })
        );
        assert_eq!(tools::accepted_label(false), "Übernommen");
        assert_eq!(tools::accepted_label(true), "Übernommen mit Änderungen");
        assert_eq!(tools::rejected_label(" "), "Verworfen");
        assert_eq!(tools::rejected_label(" zu breit"), "Verworfen: zu breit");
    }

    fn database_with_bugfix() -> (crate::db::Database, PromptTemplate) {
        let database = database();
        let saved = create(&database, &PromptTemplateInput::from(&bugfix()), |_| {})
            .expect("angelegt")
            .prompt_template
            .expect("ohne Meldung");
        (database, saved)
    }

    fn stale_error(name: &str) -> Vec<PromptTemplateError> {
        vec![error(
            PromptTemplateField::Name,
            &format!(
                "Die Vorlage „{name}“ wurde seit dem Vorschlag geändert oder gelöscht. Verwirf den Vorschlag."
            ),
        )]
    }

    #[test]
    fn a_proposal_stores_nothing_until_accepted_and_then_saves_like_the_editor() {
        let (database, base) = database_with_bugfix();
        let templates = list(&database).expect("Liste");
        let (_, wanted) =
            tools::update_input(&json!({ "target": "Bugfix", "text": "Neu." }), &templates)
                .expect("Ziel gefunden");
        let proposal = tools::checked(&wanted, &templates, Some(&base.id)).expect("gültig");
        assert_eq!(
            list(&database).expect("Liste"),
            templates,
            "der Vorschlag speichert nichts"
        );
        let events = Cell::new(0);
        let saved = update_if_unchanged(&database, &base, &proposal, |_| {
            events.set(events.get() + 1);
        })
        .expect("gespeichert");
        assert_eq!(saved.errors, Vec::new());
        assert_eq!(events.get(), 1, "das Ereignis geht nach dem Speichern");
        assert_eq!(
            list(&database).expect("Liste"),
            vec![PromptTemplate {
                text: "Neu.".to_owned(),
                ..base.clone()
            }]
        );
        // Eine Syntax, die der Editor nach dem Bearbeiten nicht mehr gesperrt hätte, prüft der Core trotzdem.
        let current = list(&database).expect("Liste")[0].clone();
        let broken =
            update_if_unchanged(&database, &current, &input("Bugfix", "{{#a}}", ""), |_| {
                events.set(events.get() + 1);
            })
            .expect("ohne Fehler");
        assert_eq!(
            broken.errors,
            vec![error(
                PromptTemplateField::Text,
                "Zeile 1, Spalte 1: Der Abschnitt „a“ ist nicht geschlossen."
            )]
        );
        assert_eq!(events.get(), 1, "kein Ereignis ohne Speichern");
    }

    #[test]
    fn a_stale_update_proposal_is_refused_when_the_template_changed_or_was_deleted() {
        let (database, base) = database_with_bugfix();
        let proposal = PromptTemplateInput {
            text: "Vorschlag.".to_owned(),
            ..PromptTemplateInput::from(&base)
        };
        let by_hand = PromptTemplateInput {
            text: "Von Hand geändert.".to_owned(),
            ..PromptTemplateInput::from(&base)
        };
        update(&database, &base.id, &by_hand, |_| {}).expect("geändert");
        let events = Cell::new(0);
        let refused = update_if_unchanged(&database, &base, &proposal, |_| {
            events.set(events.get() + 1);
        })
        .expect("ohne Fehler");
        assert_eq!(
            refused,
            PromptTemplateSaved {
                prompt_template: None,
                errors: stale_error("Bugfix"),
            }
        );
        assert_eq!(events.get(), 0);
        assert_eq!(
            list(&database).expect("Liste")[0].text,
            "Von Hand geändert."
        );

        let (database, base) = database_with_bugfix();
        delete(&database, &base.id, |_| {}).expect("gelöscht");
        let refused =
            update_if_unchanged(&database, &base, &proposal, |_| {}).expect("ohne Fehler");
        assert_eq!(refused.errors, stale_error("Bugfix"));
        assert_eq!(
            list(&database).expect("Liste"),
            Vec::new(),
            "nichts neu angelegt"
        );
    }

    #[test]
    fn chat_answer_refuses_a_prompt_template_proposal_box() {
        let request = ToolRequest::PromptTemplate {
            base: None,
            proposal: Box::new(input("Refactoring", "Text", "")),
        };
        for answer in [
            QuestionAnswer::Allow,
            QuestionAnswer::Deny {
                message: "nein".to_owned(),
            },
            QuestionAnswer::Edit {
                message: "anders".to_owned(),
                model: None,
                effort: None,
            },
            QuestionAnswer::Options {
                answers: vec!["Übernehmen".to_owned()],
            },
        ] {
            assert!(
                matches!(
                    tool_answer_label(&request, &answer),
                    Err(CommandError::Internal(message)) if message
                        == "Einen Vorschlag für eine Prompt-Vorlage beantwortet nur \
                            prompt_template_proposal_answer."
                ),
                "{answer:?} übernimmt am Speichern vorbei"
            );
        }
    }

    #[test]
    fn the_proposal_answer_command_is_class_c_with_session_id() {
        use crate::remote::access::{Class, IdKind, TargetId, class_of, ids_of};
        assert_eq!(
            class_of("prompt_template_proposal_answer"),
            Some(Class::HostWrite)
        );
        assert_eq!(
            ids_of(
                "prompt_template_proposal_answer",
                &json!({ "sessionId": "session", "requestId": "tool-1" })
            ),
            vec![TargetId {
                kind: IdKind::Session,
                value: "session".to_owned(),
            }]
        );
    }
}
