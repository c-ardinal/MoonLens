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
