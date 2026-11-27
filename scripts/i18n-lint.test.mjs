import { test } from 'node:test';
import assert from 'node:assert/strict';
import { lintDict, lintKeys, lintLocale, scanFile } from './i18n-lint.mjs';

const svelte = (text) => scanFile(text, 'src/a.svelte');
const ts = (text) => scanFile(text, 'src/a.ts');
const rust = (text) => scanFile(text, 'src-tauri/src/a.rs');

test('マークアップへの直接記述を検出（本文・属性値）', () => {
  const r = svelte('<button title="保存する">保存</button>');
  assert.deepEqual(r.issues.map((i) => [i.rule, i.text]), [['raw', '保存する'], ['raw', '保存']]);
});

test('キー ID の文字列リテラルは照合の対象（t() の引数・データ表）', () => {
  const r = svelte("<script lang=\"ts\">const rows = [{ label: 'history:title' }];</script><p>{t('common:settings')}</p>");
  assert.deepEqual(r.issues, []);
  assert.deepEqual(r.keys.map((k) => k.text).sort(), ['common:settings', 'history:title']);
});

test('日本語の文字列リテラル・テンプレート文字列を検出', () => {
  const r = ts("const a = '保存'; const b = `件数`; const c = `${n} 件`; const d = `${n} items`; const e = 'appearance.theme';");
  assert.deepEqual(r.issues.map((i) => [i.rule, i.text]), [['literal', '保存'], ['literal', '件数'], ['literal', '${…} 件']]);
  assert.deepEqual(r.keys, []);
});

test('コメント・英数字のみの文字列は対象外', () => {
  const r = svelte("<script lang=\"ts\">// 注釈\nconst a = 'abc';</script><!-- 注釈 --><p>abc</p>");
  assert.deepEqual([r.issues, r.keys], [[], []]);
});

test('i18n-ignore：直前行・同一行の指定で除外', () => {
  const r = ts("// i18n-ignore\nconst a = '禁則文字';\nconst b = '言語名'; // i18n-ignore\nconst c = '対象';\nconst d = 'ocr:ppocr'; // i18n-ignore");
  assert.deepEqual(r.issues.map((i) => i.text), ['対象']);
  assert.deepEqual(r.keys, []);
});

test('言語ファイル：空欄・キーの形式・順序・言語名の欠落を検出', () => {
  const rules = (d) => lintDict({ '@language': '日本語', ...d }).map((v) => v.rule);
  assert.deepEqual(rules({ 'common:save': '保存', 'common:settings': '設定' }), []);
  assert.deepEqual(rules({ 'common:save': '' }), ['value']);
  assert.deepEqual(rules({ '保存': '保存' }), ['value']);
  assert.deepEqual(rules({ 'common:settings': '設定', 'common:save': '保存' }), ['order']);
  assert.deepEqual(lintDict({ 'common:save': '保存' }).map((v) => [v.rule, v.text]), [['value', '@language']]);
});

test('他言語の言語ファイル：日本語の混入・{name} の不一致を基準の言語と照合', () => {
  const source = { '@language': '日本語', 'common:save': '保存', 'history:count': '{n} 件' };
  const rules = (d) => lintDict({ '@language': 'English', ...d }, source).map((v) => v.rule);
  assert.deepEqual(rules({ 'common:save': 'Save', 'history:count': '{n} items' }), []);
  assert.deepEqual(rules({ 'common:save': '保存 Save' }), ['value']);
  assert.deepEqual(rules({ 'history:count': '{count} items' }), ['value']);
});

test('Rust：tr!・Text::new・text の第 1 引数はキー ID（コメント行は対象外）', () => {
  const r = rust(
    '//! tr!("doc:example");\nlet a = tr!("ipc:read_failed", e = e);\nconst N: Text = Text::new("models:note");\nlet s = crate::i18n::text("common:list_separator");\nlet p = "プロンプト";'
  );
  assert.deepEqual(r.keys.map((k) => [k.text, k.args]), [
    ['ipc:read_failed', ['e']],
    ['models:note', null],
    ['common:list_separator', null]
  ]);
});

test('ソース中のキー ID：未登録・tr! の引数名と {name} の不一致を検出', () => {
  const source = { 'ipc:read_failed': '読込失敗：{e}', 'common:done': '完了' };
  const rules = (text) => lintKeys(rust(text).keys.map((k) => ({ file: 'a.rs', ...k })), source).map((f) => [f.rule, f.text]);
  assert.deepEqual(rules('tr!("ipc:read_failed", e = x.join(", "));\ntr!(\n    "common:done",\n);'), []);
  assert.deepEqual(rules('tr!("ipc:read_failed");'), [['args', 'ipc:read_failed']]);
  assert.deepEqual(rules('tr!("common:done", n = f(1, 2));'), [['args', 'common:done']]);
  assert.deepEqual(rules('tr!("ipc:no_such_key");'), [['missing', 'ipc:no_such_key']]);
});

test('言語ファイルの過不足：基準の言語に対する未登録、ソースに対する未使用を検出', () => {
  const source = { '@language': '日本語', 'common:save': '保存', 'common:settings': '設定' };
  const used = new Set(['common:save', 'common:settings']);
  const found = lintLocale('locales/fr.json', { '@language': 'Français', 'common:old': 'Ancien', 'common:save': 'Enregistrer' }, used, source);
  assert.deepEqual(found.map((f) => [f.rule, f.text]), [
    ['missing', 'common:settings'],
    ['unused', 'common:old']
  ]);
});
