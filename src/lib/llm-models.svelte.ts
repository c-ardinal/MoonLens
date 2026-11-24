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
  data = $state<LlmModels>({ selected: '', assistant: '', dir: '', models: [] });
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

  /** 取得後、該当用途の使用モデルが未導入なら自動で割当。 */
  async download(id: string, role: LlmRole) {
    this.busy = { ...this.busy, [id]: true };
    try {
      await installLlmModel(id);
      const m = this.data.models.find((x) => x.id === id);
      const installed = (sel: string) => this.data.models.find((x) => x.id === sel)?.installed;
      if ((m?.style === 'pfml3' || role === 'translate') && !installed(this.data.selected)) {
        await selectLlmModel(id, 'translate');
      }
      if (m?.style === 'chat_template' && !installed(this.data.assistant)) {
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
