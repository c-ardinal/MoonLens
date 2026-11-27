import { describe, expect, it } from 'vitest';
import { SUBTITLE_DOCKS, SourceLinker, sourceAfterDetach, sourceForTarget } from './subtitle-attach';
import type { AudioApp, AudioSource } from './ipc';

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

describe('SourceLinker', () => {
  const system: AudioSource = { kind: 'system' };
  const of = (pid: number): AudioSource => ({ kind: 'app', pid });
  const playing = [app(10, 'a'), app(20, 'b'), app(30, 'c')];

  /** A window on `source` whose audio list answers when `release` is called. */
  function setup(source: AudioSource = system) {
    const state = { source, sets: [] as AudioSource[] };
    const waiting: (() => void)[] = [];
    let held = false;
    const linker = new SourceLinker({
      current: async () => state.source,
      apps: () =>
        held ? new Promise((done) => waiting.push(() => done(playing))) : Promise.resolve(playing),
      set: async (next) => {
        state.sets.push(next);
        state.source = next;
      }
    });
    return {
      state,
      linker,
      hold: () => (held = true),
      release: () => waiting.splice(0).forEach((go) => go())
    };
  }

  it('switches to the attached application and back on detach', async () => {
    const { state, linker } = setup();
    await linker.follow(target(10, 'a'));
    expect(state.source).toEqual(of(10));
    await linker.follow(null);
    expect(state.source).toEqual(system);
  });

  it('a detach during the lookup leaves the source alone', async () => {
    const { state, linker, hold, release } = setup();
    hold();
    linker.follow(target(10, 'a'));
    const done = linker.follow(null);
    release();
    await done;
    expect(state.sets).toEqual([]);
  });

  it('an older target never overwrites a newer one', async () => {
    const { state, linker, hold, release } = setup();
    hold();
    linker.follow({ ...target(10, 'a'), id: 1 });
    // The first lookup is under way when the second target arrives.
    await new Promise((r) => setTimeout(r));
    const done = linker.follow({ ...target(20, 'b'), id: 2 });
    const pump = setInterval(release, 1);
    await done;
    clearInterval(pump);
    expect(state.sets).toEqual([of(20)]);
    await linker.follow(null);
    expect(state.source).toEqual(system);
  });

  it('a source chosen by hand becomes what the next detach restores', async () => {
    const { state, linker } = setup();
    await linker.follow({ ...target(10, 'a'), id: 1 });
    state.source = of(20); // the user picks another application
    await linker.follow({ ...target(30, 'c'), id: 3 });
    expect(state.source).toEqual(of(30));
    await linker.follow(null);
    expect(state.source).toEqual(of(20));
  });

  it('moving on to another target keeps the source from before the first', async () => {
    const { state, linker } = setup();
    await linker.follow({ ...target(10, 'a'), id: 1 });
    await linker.follow({ ...target(30, 'c'), id: 3 });
    await linker.follow(null);
    expect(state.source).toEqual(system);
  });
});
