import { afterEach, describe, expect, it } from 'vitest';
import { LOCALES, locale, onLocaleChange, resolveLocale, setLocale, sourceText, t, textEm } from './index';

afterEach(() => setLocale('ja'));

describe('resolveLocale', () => {
  it('明示指定を優先', () => {
    expect(resolveLocale('ja', 'en-US')).toBe('ja');
    expect(resolveLocale('en', 'ja-JP')).toBe('en');
  });
  it('auto：日本語環境は日本語、他は英語', () => {
    expect(resolveLocale('auto', 'ja-JP')).toBe('ja');
    expect(resolveLocale('auto', 'ja')).toBe('ja');
    expect(resolveLocale('auto', 'en-US')).toBe('en');
    expect(resolveLocale('auto', 'zh-CN')).toBe('en');
    expect(resolveLocale(undefined, 'fr')).toBe('en');
  });
  it('言語ファイルの無い言語コードは auto と同一', () => {
    expect(resolveLocale('xx', 'ja-JP')).toBe('ja');
    expect(resolveLocale('xx', 'en-US')).toBe('en');
  });
});

describe('t', () => {
  it('キー ID の文言を表示言語で取得', () => {
    expect(t('common:settings')).toBe('設定');
    setLocale('en');
    expect(locale()).toBe('en');
    expect(t('common:settings')).toBe('Settings');
  });
  it('{name} を値へ置換（全言語）', () => {
    expect(t('models:n_threads', { n: 8 })).toBe('8 スレッド');
    setLocale('en');
    expect(t('models:n_threads', { n: 8 })).toBe('8 threads');
  });
  it('未登録のキーはキー ID のまま表示', () => {
    setLocale('en');
    expect(t('no:such_key')).toBe('no:such_key');
  });
  it('同一の日本語でも、キー単位で訳し分け', () => {
    expect(t('common:translate_action')).toBe(t('common:translation'));
    setLocale('en');
    expect(t('common:translate_action')).toBe('Translate');
    expect(t('common:translation')).toBe('Translation');
  });
});

describe('sourceText', () => {
  it('表示言語に依存せず日本語の文言を取得', () => {
    setLocale('en');
    expect(sourceText('common:settings')).toBe('設定');
  });
});

describe('setLocale', () => {
  it('変更時のみ通知', () => {
    const seen: string[] = [];
    const off = onLocaleChange((l) => seen.push(l));
    setLocale('ja');
    setLocale('en');
    setLocale('en');
    off();
    setLocale('ja');
    expect(seen).toEqual(['en']);
  });
});

describe('textEm', () => {
  it('全角 1、半角 0.6', () => {
    expect(textEm('日本語')).toBe(3);
    expect(textEm('Japanese')).toBe(4.8);
  });
});

describe('LOCALES', () => {
  it('locales/ の全言語ファイルを列挙（基準の言語が先頭、言語名は各ファイルの @language）', () => {
    expect(LOCALES[0]).toEqual({ code: 'ja', name: '日本語' });
    expect(LOCALES).toContainEqual({ code: 'en', name: 'English' });
  });
});
