import { test } from 'node:test';
import assert from 'node:assert/strict';
import { lintText } from './copy-lint.mjs';

const words = (text, file = 'src/a.ts') => lintText(text, file).map((v) => v.word);

test('禁止語はコメント内も検出', () => {
  assert.deepEqual(words("// 窓の高さ\nconst a = '窓外：右';"), ['窓', '窓']);
  assert.deepEqual(words("const a = '淡色の面で表示';"), ['面']);
});

test('熟語内の「面」は許容', () => {
  assert.deepEqual(words("const a = '画面の背面・右側面';"), []);
});

test('送り仮名を伴う語を検出', () => {
  assert.deepEqual(words("const a = '空き容量';"), ['空き']);
  assert.deepEqual(words("const a = '元に戻す';"), ['戻す']);
  assert.deepEqual(words('<p>Ctrl+V で貼り付け</p>', 'src/a.svelte'), ['貼り', '付け']);
});

test('助詞・サ変活用・許容語は対象外', () => {
  assert.deepEqual(words("const a = '設定を保存し、画面を表示する。範囲に関する説明のみ';"), []);
  assert.deepEqual(words("const a = '閉じる';"), []);
});

test('平仮名の和語を検出', () => {
  assert.deepEqual(words("const a = '選択できます';"), ['できます']);
  assert.deepEqual(words("const a = 'この操作は復元不可';"), ['この']);
});

test('コメント内の送り仮名・テストモジュール・除外指定行は対象外', () => {
  assert.deepEqual(words("// 値を戻す\nconst a = 1; /* 空き */"), []);
  assert.deepEqual(words("<!-- 値を戻す -->\n<p>保存</p>", 'src/a.svelte'), []);
  assert.deepEqual(words('fn a() {}\n#[cfg(test)]\nmod tests {\n  const A: &str = "空き";\n}', 'src-tauri/src/a.rs'), []);
  assert.deepEqual(words("// copy-lint-ignore（LLM プロンプト）\nconst p = '説明してください';"), []);
});

test('違反の位置は行・桁で報告', () => {
  const [v] = lintText("const a = 1;\nconst b = '空き容量';", 'src/a.ts');
  assert.deepEqual([v.line, v.col], [2, 12]);
});
