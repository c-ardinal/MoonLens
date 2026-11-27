import { test } from 'node:test';
import assert from 'node:assert/strict';
import { lintDict, scanFile } from './i18n-lint.mjs';

const svelte = (text) => scanFile(text, 'src/a.svelte');
const ts = (text) => scanFile(text, 'src/a.ts');

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

test('辞書：空欄・日本語混入・{name} 不一致・順序を検出', () => {
  const rules = (d) => lintDict(d).map((v) => v.rule);
  assert.deepEqual(rules({ '保存': 'Save', '設定': 'Settings' }), []);
  assert.deepEqual(rules({ '保存': '' }), ['value']);
  assert.deepEqual(rules({ '保存': '保存 Save' }), ['value']);
  assert.deepEqual(rules({ '{n} 件': '{count} items' }), ['value']);
  assert.deepEqual(rules({ '設定': 'Settings', '保存': 'Save' }), ['order']);
});
