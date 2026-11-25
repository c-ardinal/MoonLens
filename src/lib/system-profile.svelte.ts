/**
 * PC 構成と機能別の推奨モデルの共有状態。初回セットアップと設定画面の
 * モデル一覧（推奨表示）が同一の診断結果を参照。診断は初回参照時の 1 回のみ。
 */
import { getSystemProfile, type Recommendation, type SystemReport } from '$lib/ipc';

class SystemProfileStore {
  report = $state<SystemReport | null>(null);
  private pending: Promise<SystemReport | null> | null = null;

  /** 診断結果（失敗時 null）。2 回目以降は前回結果を返却。 */
  load(): Promise<SystemReport | null> {
    this.pending ??= getSystemProfile()
      .then((r) => (this.report = r))
      .catch(() => null);
    return this.pending;
  }

  get recommended(): Recommendation | null {
    return this.report?.recommended ?? null;
  }
}

export const systemProfile = new SystemProfileStore();
