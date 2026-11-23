// Selbsttest des Zeilen-Diffs im Kasten eines Vorschlags: `node src/features/prompttemplates/lineDiff.selftest.ts`
// (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag.
import { lineDiff } from './lineDiff.ts';
import type { DiffLine } from './lineDiff.ts';

function same(text: string): DiffLine {
  return { kind: 'same', text };
}

function removed(text: string): DiffLine {
  return { kind: 'removed', text };
}

function added(text: string): DiffLine {
  return { kind: 'added', text };
}

const cases: { name: string; actual: () => unknown; expected: unknown }[] = [
  {
    name: 'equal_texts_are_all_same',
    actual: () => lineDiff('a\nb\nc', 'a\nb\nc'),
    expected: [same('a'), same('b'), same('c')],
  },
  {
    name: 'two_empty_texts_have_no_lines',
    actual: () => lineDiff('', ''),
    expected: [],
  },
  {
    name: 'only_added_lines_from_an_empty_text',
    actual: () => lineDiff('', 'a\nb'),
    expected: [added('a'), added('b')],
  },
  {
    name: 'only_added_lines_at_the_end',
    actual: () => lineDiff('a', 'a\nb\nc'),
    expected: [same('a'), added('b'), added('c')],
  },
  {
    name: 'only_removed_lines_to_an_empty_text',
    actual: () => lineDiff('a\nb', ''),
    expected: [removed('a'), removed('b')],
  },
  {
    name: 'only_removed_lines_in_the_middle',
    actual: () => lineDiff('a\nb\nc\nd', 'a\nd'),
    expected: [same('a'), removed('b'), removed('c'), same('d')],
  },
  {
    name: 'a_changed_middle_line_is_removed_then_added_between_context',
    actual: () => lineDiff('a\nalt\nc', 'a\nneu\nc'),
    expected: [same('a'), removed('alt'), added('neu'), same('c')],
  },
  {
    name: 'a_changed_block_lists_all_removed_before_all_added',
    actual: () => lineDiff('a\nx1\nx2\nc', 'a\ny1\ny2\nc'),
    expected: [same('a'), removed('x1'), removed('x2'), added('y1'), added('y2'), same('c')],
  },
  {
    name: 'the_example_of_picture_7_keeps_the_unchanged_section_as_context',
    actual: () =>
      lineDiff(
        '{{#Mit Test}}\nTest.\n{{/Mit Test}}\n{{^Mit Test}}\nKein Test.\n{{/Mit Test}}',
        '{{#Mit Test}}\nTest, rot.\n{{/Mit Test}}\n{{^Mit Test}}\nKein Test.\n{{/Mit Test}}\n{{#Mit Review}}\nReview.\n{{/Mit Review}}',
      ),
    expected: [
      same('{{#Mit Test}}'),
      removed('Test.'),
      added('Test, rot.'),
      same('{{/Mit Test}}'),
      same('{{^Mit Test}}'),
      same('Kein Test.'),
      same('{{/Mit Test}}'),
      added('{{#Mit Review}}'),
      added('Review.'),
      added('{{/Mit Review}}'),
    ],
  },
  {
    name: 'empty_lines_are_lines_of_their_own',
    actual: () => lineDiff('a\n\nb', 'a\nb\n\n'),
    expected: [same('a'), removed(''), same('b'), added(''), added('')],
  },
  {
    name: 'a_trailing_newline_against_none_is_one_empty_line',
    actual: () => [lineDiff('a\n', 'a'), lineDiff('a', 'a\n')],
    expected: [
      [same('a'), removed('')],
      [same('a'), added('')],
    ],
  },
  {
    name: 'a_repeated_line_matches_once_and_the_rest_is_a_change',
    actual: () => lineDiff('x\nx\nx', 'x'),
    expected: [same('x'), removed('x'), removed('x')],
  },
  {
    name: 'a_repeated_line_after_matches_once_and_the_rest_is_added',
    actual: () => lineDiff('x', 'x\nx\nx'),
    expected: [same('x'), added('x'), added('x')],
  },
  {
    name: 'an_ambiguous_middle_lists_removed_before_added',
    actual: () => lineDiff('b\nc\nc\nc', 'a\nc\nb\na'),
    expected: [
      removed('b'),
      removed('c'),
      removed('c'),
      added('a'),
      same('c'),
      added('b'),
      added('a'),
    ],
  },
  {
    name: 'a_moved_line_is_removed_at_one_place_and_added_at_the_other',
    actual: () => lineDiff('a\nb\nc', 'b\nc\na'),
    expected: [removed('a'), same('b'), same('c'), added('a')],
  },
];

let failed = 0;
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
