import { afterEach, describe, expect, it } from 'vitest';
import { LOCALES, locale, onLocaleChange, resolveLocale, setLocale, t, textEm } from './index';

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
  it('日本語表示は原文、英語表示は辞書の訳文', () => {
    expect(t('設定')).toBe('設定');
    setLocale('en');
    expect(locale()).toBe('en');
    expect(t('設定')).toBe('Settings');
  });
  it('{name} を値へ置換（両言語）', () => {
    expect(t('{n} スレッド', { n: 8 })).toBe('8 スレッド');
    setLocale('en');
    expect(t('{n} スレッド', { n: 8 })).toBe('8 threads');
  });
  it('未登録の文言は原文で代替', () => {
    setLocale('en');
    expect(t('辞書未登録の文言')).toBe('辞書未登録の文言');
  });
  it('#文脈：日本語表示は除去、英語表示は文脈別の訳文', () => {
    expect(t('翻訳#action')).toBe('翻訳');
    setLocale('en');
    expect(t('翻訳#action')).toBe('Translate');
    expect(t('翻訳')).toBe('Translation');
    // 文脈付きの訳文が未登録：文脈を除去した原文
    expect(t('辞書未登録の文言#ctx')).toBe('辞書未登録の文言');
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
  it('原文の言語と、locales/ の全言語ファイルを列挙（言語名は各ファイルの @language）', () => {
    expect(LOCALES[0]).toEqual({ code: 'ja', name: '日本語' });
    expect(LOCALES).toContainEqual({ code: 'en', name: 'English' });
  });
});
