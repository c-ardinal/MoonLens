import { describe, expect, it } from 'vitest';
import {
  addApp,
  appKey,
  compactAppGlossaries,
  glossaryOf,
  insertApp,
  isBlankApp,
  removeApp,
  setGlossary,
  type AppGlossary
} from './app-glossary';

const term = (source: string, target: string) => ({ source, target, lang: '' });

describe('appKey', () => {
  // Same table as `key_is_the_lowercased_base_name_without_exe` in
  // src-tauri/src/app_identity.rs: both sides must agree.
  it.each([
    ['Game.exe', 'game'],
    ['GAME.EXE', 'game'],
    ['game', 'game'],
    ['C:\\Program Files\\Some Game\\Game.exe', 'game'],
    ['/usr/lib/firefox/firefox', 'firefox'],
    ['  Visual Novel.exe  ', 'visual novel'],
    ['ÉDITEUR.exe', 'éditeur'],
    ['archive.tar', 'archive.tar'],
    ['', ''],
    ['.exe', ''],
    ['dir/', '']
  ])('%j → %j', (name, key) => {
    expect(appKey(name)).toBe(key);
  });
});

describe('application list', () => {
  it('adds an application under its key and keeps the spelling for display', () => {
    const { list, key } = addApp([], 'D:\\Games\\MyGame.EXE');
    expect(key).toBe('mygame');
    expect(list).toEqual([{ app: 'mygame', name: 'MyGame', glossary: [] }]);
  });

  it('does not list an application twice', () => {
    const first = addApp([], 'Game.exe').list;
    const again = addApp(first, 'game');
    expect(again.key).toBe('game');
    expect(again.list).toBe(first);
  });

  it('adds nothing for a name that names nothing', () => {
    const list: AppGlossary[] = [];
    expect(addApp(list, '  .exe ')).toEqual({ list, key: null });
  });

  it('edits the entries of one application only', () => {
    let list = addApp(addApp([], 'a.exe').list, 'b.exe').list;
    list = setGlossary(list, 'b', [term('Ether', 'エーテル')]);
    expect(glossaryOf(list, 'a')).toEqual([]);
    expect(glossaryOf(list, 'b')).toEqual([term('Ether', 'エーテル')]);
    expect(glossaryOf(list, 'c')).toBeUndefined();
  });

  it('puts a removed application back where it was, once', () => {
    const list = addApp(addApp(addApp([], 'a').list, 'b').list, 'c').list;
    const removed = list[1];
    const without = removeApp(list, 'b');
    expect(without.map((a) => a.app)).toEqual(['a', 'c']);
    const back = insertApp(without, 1, removed);
    expect(back).toEqual(list);
    expect(insertApp(back, 0, removed)).toBe(back);
  });

  it('drops blank rows on save and keeps the applications', () => {
    const list = [{ app: 'a', name: 'A', glossary: [term('', ' '), term('Ether', 'エーテル')] }, { app: 'b', name: 'B', glossary: [term('', '')] }];
    expect(compactAppGlossaries(list)).toEqual([
      { app: 'a', name: 'A', glossary: [term('Ether', 'エーテル')] },
      { app: 'b', name: 'B', glossary: [] }
    ]);
    expect(compactAppGlossaries(null)).toEqual([]);
    expect(isBlankApp(list[1])).toBe(true);
    expect(isBlankApp(list[0])).toBe(false);
  });
});
