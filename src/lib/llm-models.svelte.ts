/**
 * 翻訳・説明用モデルの共有状態。設定画面の用途別パネル（翻訳／説明・要約）が
 * 同一の一覧・取得進捗を参照し、取得・削除の結果を両パネルへ即時反映。
 */
import {
  installLlmModel,
  listLlmModels,
  onLlmProgress,
  selectLlmModel,
  type LlmModels,
  type LlmProgress,
  type LlmRole
} from '$lib/ipc';

class LlmModelStore {
  data = $state<LlmModels>({ selected: '', assistant: '', dir: '', models: [], accepted_licenses: [] });
  progress = $state<Record<string, LlmProgress>>({});
  busy = $state<Record<string, boolean>>({});
  private users = 0;
  private unlisten: (() => void) | undefined;

  async refresh() {
    this.data = await listLlmModels();
  }

  /** 表示側の登録：初回で進捗の購読を開始、最後の解除で停止。 */
  async attach() {
    this.users++;
    if (this.users === 1) {
      const off = await onLlmProgress((p) => {
        this.progress = { ...this.progress, [p.id]: p };
      });
      if (this.users > 0 && !this.unlisten) this.unlisten = off;
      else off();
    }
  }

  detach() {
    this.users = Math.max(0, this.users - 1);
    if (this.users === 0) {
      this.unlisten?.();
      this.unlisten = undefined;
    }
  }

  /**
   * 取得。select 指定時は完了後に該当用途の選択を切替。未指定でも、
   * 選択中モデルが未取得の用途へは自動割当（各機能のモデル不在を回避）。
   */
  async download(id: string, role: LlmRole, select = false) {
    this.busy = { ...this.busy, [id]: true };
    try {
      await installLlmModel(id);
      await this.refresh().catch(() => {});
      const m = this.data.models.find((x) => x.id === id);
      const installed = (sel: string) => this.data.models.find((x) => x.id === sel)?.installed;
      if (select) {
        await selectLlmModel(id, role);
      } else if ((m?.style === 'pfml3' || role === 'translate') && !installed(this.data.selected)) {
        await selectLlmModel(id, 'translate');
      }
      if (m?.style === 'chat_template' && !installed(this.data.assistant) && !(select && role === 'assistant')) {
        await selectLlmModel(id, 'assistant');
      }
    } finally {
      this.busy = { ...this.busy, [id]: false };
      const { [id]: _done, ...rest } = this.progress;
      this.progress = rest;
      await this.refresh().catch(() => {});
    }
  }
}

export const llmModels = new LlmModelStore();
