import { test } from 'node:test';
import assert from 'node:assert/strict';
import { lintDict, lintLocale, scanFile } from './i18n-lint.mjs';

const svelte = (text) => scanFile(text, 'src/a.svelte');
const ts = (text) => scanFile(text, 'src/a.ts');
const rust = (text) => scanFile(text, 'src-tauri/src/a.rs');

test('マークアップへの直接記述を検出（本文・属性値）', () => {
  const r = svelte('<button title="保存する">保存</button>');
  assert.deepEqual(r.issues.map((i) => [i.rule, i.text]), [['raw', '保存する'], ['raw', '保存']]);
});

test('t() の引数・データ表の文字列リテラルは辞書照合の対象', () => {
  const r = svelte("<script lang=\"ts\">const rows = [{ label: '履歴' }];</script><p>{t('設定')}</p>");
  assert.deepEqual(r.issues, []);
  assert.deepEqual(r.literals.map((l) => l.text).sort(), ['履歴', '設定'].sort());
});

test('コメント・英数字のみの文字列は対象外', () => {
  const r = svelte("<script lang=\"ts\">// 注釈\nconst a = 'abc';</script><!-- 注釈 --><p>abc</p>");
  assert.deepEqual([r.issues, r.literals], [[], []]);
});

test('日本語を含むテンプレート文字列への埋込みを検出', () => {
  const r = ts('const a = `${n} 件`; const b = `件数`; const c = `${n} items`;');
  assert.deepEqual(r.issues.map((i) => i.rule), ['template']);
  assert.deepEqual(r.literals.map((l) => l.text), ['件数']);
});

test('i18n-ignore：直前行・同一行の指定で除外', () => {
  const r = ts("// i18n-ignore\nconst a = '禁則文字';\nconst b = '言語名'; // i18n-ignore\nconst c = '対象';");
  assert.deepEqual(r.literals.map((l) => l.text), ['対象']);
});

test('言語ファイル：空欄・日本語混入・{name} 不一致・順序・言語名の欠落を検出', () => {
  const rules = (d) => lintDict({ '@language': 'English', ...d }).map((v) => v.rule);
  assert.deepEqual(rules({ '保存': 'Save', '設定': 'Settings' }), []);
  assert.deepEqual(rules({ '保存': '' }), ['value']);
  assert.deepEqual(rules({ '保存': '保存 Save' }), ['value']);
  assert.deepEqual(rules({ '{n} 件': '{count} items' }), ['value']);
  assert.deepEqual(rules({ '設定': 'Settings', '保存': 'Save' }), ['order']);
  assert.deepEqual(lintDict({ '保存': 'Save' }).map((v) => [v.rule, v.text]), [['value', '@language']]);
});

test('Rust：tr!・Text::new・text の原文は照合の対象（コメント行は対象外）', () => {
  const r = rust(
    '//! tr!("使用例");\nlet a = tr!("読込失敗：{e}", e = e);\nconst N: Text = Text::new("OCR");\nlet s = crate::i18n::text("、");\nlet p = "プロンプト";'
  );
  assert.deepEqual(r.literals.map((l) => l.text), ['読込失敗：{e}', 'OCR', '、']);
  assert.deepEqual(r.issues, []);
});

test('Rust：tr! の引数名と {name} の不一致を検出', () => {
  const issues = (text) => rust(text).issues.map((i) => [i.rule, i.text]);
  assert.deepEqual(issues('tr!("{a}／{b}", a = x.join(", "), b = f(1, 2));'), []);
  assert.deepEqual(issues('tr!("読込失敗：{e}");'), [['args', '読込失敗：{e}']]);
  assert.deepEqual(issues('tr!("完了", n = 1);'), [['args', '完了']]);
  assert.deepEqual(issues('tr!(\n    "複数行：{e}",\n    e = e,\n);'), []);
});

test('言語ファイルと走査結果の照合：未登録・未使用を言語ファイル単位で検出', () => {
  const scan = { literals: [{ file: 'src/a.ts', line: 1, text: '保存' }], used: new Set(['保存']) };
  const found = lintLocale('locales/fr.json', { '@language': 'Français', '設定': 'Paramètres' }, scan);
  assert.deepEqual(found.map((f) => [f.file, f.rule, f.text]), [
    ['src/a.ts', 'missing', '保存'],
    ['locales/fr.json', 'unused', '設定']
  ]);
});
