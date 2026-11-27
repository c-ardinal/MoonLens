/**
 * 音声翻訳ウィンドウの吸着：配置の選択肢、吸着先アプリへの音源連動。
 */
import type { AttachDock, AttachTarget } from './attach';
import type { AudioApp, AudioSource } from './ipc';

/** 選択可能な配置（先頭：新規吸着時の配置）。 */
export const SUBTITLE_DOCKS: { value: AttachDock; label: string; title: string }[] = [
  { value: 'inside', label: '内', title: '吸着先の内側下端' },
  { value: 'bottom', label: '下', title: '吸着先の下側（外側）' },
  { value: 'top', label: '上', title: '吸着先の上側（外側）' },
  { value: 'left', label: '左', title: '吸着先の左側（外側）' },
  { value: 'right', label: '右', title: '吸着先の右側（外側）' },
  { value: 'free', label: '自由', title: '現在位置を保持（吸着先との相対位置で追従）' }
];

const stem = (name: string) => name.trim().toLowerCase().replace(/\.exe$/, '');

/**
 * 吸着先のアプリを音源とする指定。音声出力中の一覧から同一プロセス、
 * 不在時は同名アプリ（1 件のみの場合）を採用。該当なし：null（音源は変更なし）。
 */
export function sourceForTarget(target: AttachTarget, apps: AudioApp[]): AudioSource | null {
  const byPid = apps.find((a) => a.pid === target.pid);
  if (byPid) return { kind: 'app', pid: byPid.pid };
  const name = stem(target.app);
  if (!name) return null;
  const byName = apps.filter((a) => stem(a.name) === name);
  return byName.length === 1 ? { kind: 'app', pid: byName[0].pid } : null;
}

const same = (a: AudioSource, b: AudioSource) =>
  a.kind === b.kind && (a.kind !== 'app' || a.pid === (b as { pid: number }).pid);

/** 吸着による音源連動の記録（解除時の復帰用）。 */
export interface SourceLink {
  /** 吸着時に設定した音源。 */
  linked: AudioSource;
  /** 吸着前の音源。 */
  before: AudioSource;
}

/**
 * 吸着解除時に復帰する音源。吸着後に利用者が音源を変更済みの場合は null（変更を維持）。
 */
export function sourceAfterDetach(link: SourceLink | null, current: AudioSource): AudioSource | null {
  if (!link || !same(link.linked, current) || same(link.before, current)) return null;
  return link.before;
}

/** 吸着前の音源として記録する値。連動後に利用者が音源を変更済みの場合は現在の音源。 */
export function sourceBeforeLink(link: SourceLink | null, current: AudioSource): AudioSource {
  return link && same(link.linked, current) ? link.before : current;
}

export interface SourceLinkerDeps {
  /** ウィンドウの現在の音源。 */
  current(): Promise<AudioSource>;
  /** 音声出力中のアプリ一覧。 */
  apps(): Promise<AudioApp[]>;
  set(source: AudioSource): Promise<void>;
}

/**
 * 吸着先の変化へ音源を連動。変更は到着順に 1 件ずつ処理し、
 * 処理開始時点で吸着先が更新済みの要求は破棄（旧要求による音源変更の防止）。
 */
export class SourceLinker {
  private link: SourceLink | null = null;
  private latest: AttachTarget | null = null;
  private queue: Promise<void> = Promise.resolve();

  constructor(private deps: SourceLinkerDeps) {}

  /** 吸着先の変化（null：解除）。戻り値：当該要求までの処理完了。 */
  follow(target: AttachTarget | null): Promise<void> {
    this.latest = target;
    this.queue = this.queue.then(() => this.settle(target)).catch(() => {});
    return this.queue;
  }

  private stale(target: AttachTarget | null) {
    return (this.latest?.id ?? null) !== (target?.id ?? null);
  }

  private async settle(target: AttachTarget | null) {
    if (this.stale(target)) return;
    if (!target) {
      const link = this.link;
      this.link = null;
      const back = sourceAfterDetach(link, await this.deps.current());
      if (back) await this.deps.set(back);
      return;
    }
    const want = sourceForTarget(target, await this.deps.apps());
    if (!want || this.stale(target)) return;
    const current = await this.deps.current();
    if (same(want, current)) return;
    const before = sourceBeforeLink(this.link, current);
    // 取得開始の失敗：従来の音源と記録を継続。
    await this.deps.set(want);
    this.link = { linked: want, before };
  }
}
