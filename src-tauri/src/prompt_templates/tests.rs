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
