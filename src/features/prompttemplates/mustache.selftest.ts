// Selbsttest der Mustache-Teilmenge der Prompt-Vorlagen: `node src/features/prompttemplates/mustache.selftest.ts`
// (`pnpm selftest`). Felder, Füllen und jeder Fall der Fallliste, die auch der Rust-Test des Core durchgeht
// (`src-tauri/src/prompt_templates/syntax_cases.json`). Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag.
import { parseTemplate, renderTemplate } from './mustache.ts';
import type { Parsed, TemplateError, TemplateField } from './mustache.ts';
// Als JSON-Import statt über `node:fs`: `src` hat keine Node-Typen, und `node` liest ihn ebenso.
import sharedCases from '../../../src-tauri/src/prompt_templates/syntax_cases.json' with { type: 'json' };

const BUGFIX =
  '{{! Kurze Beschreibung des Fehlers }}Behebe den Fehler: {{Fehler}} in {{Betroffene Komponente}}. {{#Mit Test}}Schreibe zuerst einen Test, der den Fehler in {{Betroffene Komponente}} zeigt.{{/Mit Test}}{{^Mit Test}}Ein neuer Test ist nicht nötig.{{/Mit Test}}';

function fieldsOf(text: string, nameTemplate = ''): TemplateField[] | TemplateError {
  const parsed: Parsed = parseTemplate(text, nameTemplate);
  return parsed.ok ? parsed.fields : parsed.error;
}

function text(name: string, help: string | null = null): TemplateField {
  return { kind: 'text', name, help };
}

function flag(name: string): TemplateField {
  return { kind: 'flag', name, help: null };
}

const cases: { name: string; actual: () => unknown; expected: unknown }[] = [
  {
    name: 'fields_of_the_bugfix_example_with_help_in_order_of_first_appearance',
    actual: () => fieldsOf(BUGFIX, 'Bugfix: {{Betroffene Komponente}}'),
    expected: [
      text('Fehler', 'Kurze Beschreibung des Fehlers'),
      text('Betroffene Komponente'),
      flag('Mit Test'),
    ],
  },
  {
    name: 'a_variable_used_twice_is_one_field',
    actual: () => fieldsOf('{{a}} {{b}} {{a}} {{{b}}}'),
    expected: [text('a'), text('b')],
  },
  {
    name: 'section_and_inverted_section_of_one_name_are_one_checkbox',
    actual: () => fieldsOf('{{^x}}nein{{/x}}{{#x}}ja{{/x}}'),
    expected: [flag('x')],
  },
  {
    name: 'fields_only_in_the_name_template_come_after_those_of_the_text',
    actual: () => fieldsOf('{{b}}', '{{a}}: {{b}} {{#c}}!{{/c}}'),
    expected: [text('b'), text('a'), flag('c')],
  },
  {
    name: 'a_comment_helps_the_next_variable_even_behind_a_section_tag',
    actual: () => fieldsOf('{{! Hilfe }}{{#s}}{{x}}{{/s}}'),
    expected: [flag('s'), text('x', 'Hilfe')],
  },
  {
    name: 'a_comment_without_a_following_variable_is_not_shown',
    actual: () => fieldsOf('{{x}}{{! verloren }}'),
    expected: [text('x')],
  },
  {
    name: 'a_comment_does_not_overwrite_the_help_of_an_earlier_field',
    actual: () => fieldsOf('{{! erste }}{{x}} {{! zweite }}{{x}}'),
    expected: [text('x', 'erste')],
  },
  {
    name: 'a_comment_in_the_text_does_not_help_a_variable_of_the_name_template',
    actual: () => fieldsOf('Text {{! nur hier }}', '{{x}}'),
    expected: [text('x')],
  },
  {
    name: 'an_empty_comment_gives_no_help',
    actual: () => fieldsOf('{{!  }}{{x}}'),
    expected: [text('x')],
  },
  {
    name: 'empty_fields_stay_placeholders_in_text_and_name',
    actual: () => [
      renderTemplate('Behebe {{Fehler}} in {{ Ort }}.', { Fehler: '' }),
      renderTemplate('Bugfix: {{Ort}}', {}),
    ],
    expected: ['Behebe {{Fehler}} in {{Ort}}.', 'Bugfix: {{Ort}}'],
  },
  {
    name: 'a_variable_is_replaced_at_every_place',
    actual: () => renderTemplate('{{a}}-{{a}}-{{{a}}}', { a: 'x' }),
    expected: 'x-x-x',
  },
  {
    name: 'nothing_is_escaped',
    actual: () => renderTemplate('{{a}} {{{a}}}', { a: '<b> & "c" \'d\'' }),
    expected: '<b> & "c" \'d\' <b> & "c" \'d\'',
  },
  {
    name: 'a_multi_line_value_stays_as_typed',
    actual: () => renderTemplate('Fehler: {{a}}.', { a: 'Zeile 1\nZeile 2' }),
    expected: 'Fehler: Zeile 1\nZeile 2.',
  },
  {
    name: 'a_section_shows_only_with_the_tick_and_the_inverted_only_without',
    actual: () =>
      [true, false].map((on: boolean) =>
        renderTemplate(BUGFIX, { 'Mit Test': on, 'Betroffene Komponente': 'Leiste' }),
      ),
    expected: [
      'Behebe den Fehler: {{Fehler}} in Leiste. Schreibe zuerst einen Test, der den Fehler in Leiste zeigt.',
      'Behebe den Fehler: {{Fehler}} in Leiste. Ein neuer Test ist nicht nötig.',
    ],
  },
  {
    name: 'a_checkbox_starts_off',
    actual: () => renderTemplate('{{#a}}an{{/a}}{{^a}}aus{{/a}}', {}),
    expected: 'aus',
  },
  {
    name: 'nested_sections_need_every_tick',
    actual: () =>
      [
        { a: true, b: true },
        { a: true, b: false },
        { a: false, b: true },
      ].map((values: Record<string, boolean>) =>
        renderTemplate('[{{#a}}A{{#b}}B{{/b}}{{^b}}b{{/b}}{{/a}}]', values),
      ),
    expected: ['[AB]', '[Ab]', '[]'],
  },
  {
    name: 'a_variable_in_a_hidden_section_is_not_filled',
    actual: () => renderTemplate('{{#s}}{{x}}{{/s}}', { x: 'wert' }),
    expected: '',
  },
  {
    name: 'comments_never_reach_the_prompt',
    actual: () => renderTemplate('A{{! weg }}B', {}),
    expected: 'AB',
  },
  {
    name: 'tags_alone_on_their_line_take_the_line_with_them',
    actual: () =>
      [true, false].map((on: boolean) =>
        renderTemplate(
          'Kopf\n{{#t}}\n  Test\n{{/t}}\n  {{! Hilfe }}\n{{^t}}\nKein Test\n{{/t}}\nFuß',
          {
            t: on,
          },
        ),
      ),
    expected: ['Kopf\n  Test\nFuß', 'Kopf\nKein Test\nFuß'],
  },
  {
    name: 'a_standalone_tag_takes_a_crlf_line_and_one_at_the_end_its_indent',
    actual: () => [
      renderTemplate('a\r\n{{#t}}\r\nb\r\n{{/t}}\r\nc', { t: true }),
      renderTemplate('a\n  {{! Ende }}', {}),
    ],
    expected: ['a\r\nb\r\nc', 'a\n'],
  },
  {
    name: 'a_tag_sharing_its_line_with_text_or_a_variable_keeps_the_line',
    actual: () => [
      renderTemplate('{{#t}}Test{{/t}}\nweiter', { t: true }),
      renderTemplate('  {{#t}}{{x}}\n{{/t}}', { t: true, x: 'v' }),
    ],
    expected: ['Test\nweiter', '  v\n'],
  },
  {
    name: 'a_template_with_a_syntax_error_comes_back_unchanged',
    actual: () => renderTemplate('{{#a}}offen', { a: true }),
    expected: '{{#a}}offen',
  },
];

interface SharedCase {
  rule: string;
  text: string;
  nameTemplate: string;
  error: TemplateError | null;
}

const shared: SharedCase[] = sharedCases as unknown as SharedCase[];
for (const entry of shared) {
  cases.push({
    name: `shared_case: ${entry.rule}`,
    actual: () => {
      const parsed: Parsed = parseTemplate(entry.text, entry.nameTemplate);
      return parsed.ok ? null : parsed.error;
    },
    expected:
      entry.error === null
        ? null
        : {
            source: entry.error.source,
            message: entry.error.message,
            line: entry.error.line,
            column: entry.error.column,
          },
  });
}

let failed = 0;
if (shared.length < 20) {
  failed += 1;
  console.log(`FAIL shared_case_list_is_there: ${String(shared.length)} Fälle`);
}
for (const { name, actual, expected } of cases) {
  const result: string = JSON.stringify(actual());
  if (result === JSON.stringify(expected)) {
    console.log(`PASS ${name}`);
  } else {
    failed += 1;
    console.log(`FAIL ${name}: erwartet ${JSON.stringify(expected)}, bekommen ${result}`);
  }
}
if (failed > 0) {
  throw new Error(`${String(failed)} von ${String(cases.length)} Fällen gescheitert`);
}
console.log(`Alle ${String(cases.length)} Fälle bestanden`);
