<script lang="ts">
  /**
   * 履歴詳細の音声翻訳（記録）：行一覧・再翻訳・書出し。
   * 履歴詳細の枠内へ配置（見出し・本文・操作列の 3 部）。記録なしの履歴は `plain`（原文・訳文）を表示。
   */
  import type { Snippet } from 'svelte';
  import { onDestroy } from 'svelte';
  import { Captions, Copy, Cpu, Cloud, FileText, Languages, Trash2 } from 'lucide-svelte';
  import { copyText, notify, notifyError, undoable } from '$lib/toast.svelte';
  import { errorText } from '$lib/errors';
  import { dateLabel, kindLabel } from '$lib/history';
  import { extractText } from '$lib/result';
  import {
    exportSubtitleRecord,
    getSubtitleRecord,
    replaceSubtitleLines,
    runTranslate,
    type HistoryEntry,
    type SubtitleExportContent,
    type SubtitleExportFormat,
    type SubtitleRecord
  } from '$lib/ipc';
  import { durationLabel, groupSentences, retranslate, timeLabel } from '$lib/subtitle-record';

  let {
    entry,
    plain,
    onchanged,
    onremove
  }: {
    entry: HistoryEntry;
    /** 記録なしの履歴の本文（原文・訳文）。 */
    plain: Snippet;
    /** 記録の変更後：一覧の再読込。 */
    onchanged: () => void;
    onremove: () => void;
  } = $props();

  /** undefined：読込中、null：記録なし。 */
  let record = $state<SubtitleRecord | null | undefined>(undefined);
  let progress = $state<{ done: number; total: number } | null>(null);
  let content = $state<SubtitleExportContent>('translated');
  let ctl: AbortController | null = null;

  /** 1 時間以上の記録は時刻欄を拡幅（H:MM:SS）。 */
  const long = $derived((record?.lines.at(-1)?.end_ms ?? 0) >= 3_600_000);

  const CONTENT_LABELS: [SubtitleExportContent, string][] = [
    ['translated', '訳文'],
    ['original', '原文'],
    ['both', '併記']
  ];

  function cancel() {
    ctl?.abort();
    ctl = null;
    progress = null;
  }

  // 項目の切替で、実行中の再翻訳を中止し記録を再取得
  // 履歴の原文・訳文の変化（記録中の行追加、一覧の再読込で検出）でも再取得：
  // 表示と改版番号を最新へ更新。同一項目の再取得中は表示を維持。
  const entryId = $derived(entry.id);
  const texts = $derived(`${entry.source_text}\u0000${entry.result_text}`);
  let shown = -1;
  $effect(() => {
    const id = entryId;
    void texts;
    if (shown !== id) record = undefined;
    shown = id;
    getSubtitleRecord(id)
      .then((r) => {
        if (entry.id === id) record = r;
      })
      .catch(() => {
        if (entry.id === id) record = null;
      });
    return cancel;
  });
  onDestroy(cancel);

  async function start() {
    const rec = record;
    if (!rec || ctl) return;
    const id = entry.id;
    const before = $state.snapshot(rec.lines);
    const mine = (ctl = new AbortController());
    const total = groupSentences(before).length;
    progress = { done: 0, total };
    try {
      const { lines, failed } = await retranslate(
        before,
        async (text, history) =>
          extractText(
            await runTranslate({
              text,
              source_language: rec.source_language,
              target_language: rec.target_language,
              history
            })
          ),
        { signal: mine.signal, onProgress: (done, total) => (progress = { done, total }) }
      );
      if (failed === total) {
        notifyError('再翻訳失敗：全件の翻訳不可（変更なし）');
        return;
      }
      // 再翻訳中に記録が変化（音声翻訳の実行中）した場合は失敗し、記録中の行を保護
      const revision = await replaceSubtitleLines(id, rec.revision, lines);
      if (entry.id === id) record = { ...rec, revision, lines };
      onchanged();
      undoable({
        text: failed > 0 ? `再翻訳完了（${failed} 件は旧訳を維持）` : '再翻訳完了',
        failure: '取消失敗',
        undo: async () => {
          const restored = await replaceSubtitleLines(id, revision, before);
          if (entry.id === id) record = { ...rec, revision: restored, lines: before };
          onchanged();
        }
      });
    } catch (e) {
      if (!mine.signal.aborted) notifyError(`再翻訳失敗：${errorText(e)}`);
    } finally {
      if (ctl === mine) {
        ctl = null;
        progress = null;
      }
    }
  }

  async function save(format: SubtitleExportFormat) {
    const label = format === 'srt' ? 'SRT' : 'テキスト';
    try {
      const path = await exportSubtitleRecord(entry.id, format, content);
      if (path) notify(`${label} 出力完了：${path.split(/[\\/]/).pop()}`);
    } catch (e) {
      notifyError(`${label} 出力失敗：${errorText(e)}`);
    }
  }

  function copyTranslation() {
    const text = record?.lines
      .map((l) => l.translated)
      .filter(Boolean)
      .join('\n');
    if (text) void copyText(text, '訳文');
  }
</script>

<div class="flex flex-wrap items-center gap-2 border-b border-border px-4 py-3 text-xs text-ink-faint">
  <span class="rounded-full bg-surface-muted px-2 py-0.5">{kindLabel(entry.kind)}</span>
  <time datetime={new Date(entry.created_at * 1000).toISOString()}>{dateLabel(entry.created_at)}</time>
  {#if record}
    <span class="min-w-0 break-words rounded-full bg-surface-muted px-2 py-0.5">音源：{record.source}</span>
    <span class="tabular-nums">{durationLabel(record.lines)}</span>
  {/if}
  {#if entry.provider}
    <span class="inline-flex items-center gap-1 font-mono">
      {#if entry.via_cloud}
        <Cloud class="h-3 w-3 text-accent" />
      {:else}
        <Cpu class="h-3 w-3" />
      {/if}
      {entry.provider}
    </span>
  {/if}
</div>

{#if record}
  <ol class="scrollbar-minimal flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-4" aria-label="字幕の行" style:--tc={long ? '7.5rem' : '5rem'}>
    {#each record.lines as l (l.seq)}
      <li class="grid grid-cols-[var(--tc)_minmax(0,1fr)] gap-2">
        <time class="text-xs tabular-nums text-ink-faint">{timeLabel(l.start_ms)}</time>
        <div class="min-w-0">
          <p class="font-mono text-xs leading-relaxed break-words whitespace-pre-wrap text-ink-muted">{l.original}</p>
          {#if l.translated}
            <p class="text-sm leading-relaxed break-words whitespace-pre-wrap">{l.translated}</p>
          {/if}
        </div>
      </li>
    {/each}
  </ol>
{:else}
  <div class="scrollbar-minimal flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-4">
    {#if record === undefined}
      <p class="text-sm text-ink-muted">読込中…</p>
    {:else}
      {@render plain()}
    {/if}
  </div>
{/if}

<div class="flex flex-wrap items-center gap-2 border-t border-border px-4 py-2">
  {#if record}
    {#if progress}
      <span class="text-sm tabular-nums text-ink-muted" role="status">再翻訳中 {progress.done}/{progress.total}</span>
      <button class="sbtn" onclick={cancel}>中止</button>
    {:else}
      <button class="sbtn icon neutral" title="再翻訳" aria-label="再翻訳" onclick={start}>
        <Languages class="h-3.5 w-3.5" />
      </button>
    {/if}
    <button class="sbtn icon neutral" title="SRT 出力" aria-label="SRT 出力" onclick={() => save('srt')}>
      <Captions class="h-3.5 w-3.5" />
    </button>
    <button class="sbtn icon neutral" title="テキスト出力" aria-label="テキスト出力" onclick={() => save('txt')}>
      <FileText class="h-3.5 w-3.5" />
    </button>
    <select
      class="rounded-ml-sm border border-border bg-surface px-2 py-1.5 text-sm text-ink"
      title="出力内容"
      aria-label="出力内容"
      bind:value={content}
    >
      {#each CONTENT_LABELS as [value, label] (value)}
        <option {value}>{label}</option>
      {/each}
    </select>
    <button class="sbtn icon neutral" title="訳文をコピー" aria-label="訳文をコピー" onclick={copyTranslation}>
      <Copy class="h-3.5 w-3.5" />
    </button>
  {/if}
  <button
    class="sbtn icon ml-auto"
    title="当該履歴を削除"
    aria-label="当該履歴を削除"
    onclick={onremove}
  >
    <Trash2 class="h-3.5 w-3.5" />
  </button>
</div>
