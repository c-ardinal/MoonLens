import { describe, expect, it } from 'vitest';
import { SUBTITLE_DOCKS, sourceAfterDetach, sourceForTarget } from './subtitle-attach';
import type { AudioApp } from './ipc';

const app = (pid: number, name: string): AudioApp => ({ pid, name, title: null, playing: true });
const target = (pid: number, name: string) => ({ id: 1, pid, title: 'w', app: name });

describe('sourceForTarget', () => {
  it('takes the application with the pid of the window', () => {
    const apps = [app(10, 'vlc'), app(20, 'chrome')];
    expect(sourceForTarget(target(20, 'other.exe'), apps)).toEqual({ kind: 'app', pid: 20 });
  });

  it('falls back to the only application of the same name', () => {
    const apps = [app(10, 'vlc'), app(20, 'chrome')];
    expect(sourceForTarget(target(99, 'Chrome.exe'), apps)).toEqual({ kind: 'app', pid: 20 });
  });

  it('leaves the source alone when the name is ambiguous or unknown', () => {
    const apps = [app(10, 'chrome'), app(20, 'chrome')];
    expect(sourceForTarget(target(99, 'chrome.exe'), apps)).toBeNull();
    expect(sourceForTarget(target(99, 'notepad.exe'), apps)).toBeNull();
    expect(sourceForTarget(target(99, ''), [app(10, '')])).toBeNull();
  });
});

describe('sourceAfterDetach', () => {
  const linked = { kind: 'app', pid: 20 } as const;
  const system = { kind: 'system' } as const;

  it('goes back to the source from before the attachment', () => {
    expect(sourceAfterDetach({ linked, before: system }, linked)).toEqual(system);
  });

  it('keeps a source the user chose after attaching', () => {
    expect(sourceAfterDetach({ linked, before: system }, { kind: 'app', pid: 30 })).toBeNull();
    expect(sourceAfterDetach({ linked, before: system }, system)).toBeNull();
  });

  it('does nothing without a link', () => {
    expect(sourceAfterDetach(null, linked)).toBeNull();
    expect(sourceAfterDetach({ linked, before: linked }, linked)).toBeNull();
  });
});

describe('SUBTITLE_DOCKS', () => {
  it('offers inside first, then the four sides and free placement', () => {
    expect(SUBTITLE_DOCKS.map((d) => d.value)).toEqual([
      'inside',
      'bottom',
      'top',
      'left',
      'right',
      'free'
    ]);
  });
});
