<script lang="ts">
  import { t } from '$lib/i18n';
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
    ['translated', 'common:translated_text'],
    ['original', 'common:source'],
    ['both', 'subtitle-record-detail:both']
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
        notifyError(t('subtitle-record-detail:retranslation_failed_no_line_could_be'));
        return;
      }
      // 再翻訳中に記録が変化（音声翻訳の実行中）した場合は失敗し、記録中の行を保護
      const revision = await replaceSubtitleLines(id, rec.revision, lines);
      if (entry.id === id) record = { ...rec, revision, lines };
      onchanged();
      undoable({
        text: failed > 0 ? t('subtitle-record-detail:retranslation_complete_n_lines_kept_the', { n: failed }) : t('subtitle-record-detail:retranslation_complete'),
        failure: t('subtitle-record-detail:undo_failed'),
        undo: async () => {
          const restored = await replaceSubtitleLines(id, revision, before);
          if (entry.id === id) record = { ...rec, revision: restored, lines: before };
          onchanged();
        }
      });
    } catch (e) {
      if (!mine.signal.aborted) notifyError(t('subtitle-record-detail:retranslation_failed_error', { error: errorText(e) }));
    } finally {
      if (ctl === mine) {
        ctl = null;
        progress = null;
      }
    }
  }

  async function save(format: SubtitleExportFormat) {
    const label = format === 'srt' ? 'SRT' : t('common:text');
    try {
      const path = await exportSubtitleRecord(entry.id, format, content);
      if (path) notify(t('subtitle-record-detail:label_exported_file', { label, file: path.split(/[\\/]/).pop() ?? '' }));
    } catch (e) {
      notifyError(t('subtitle-record-detail:label_export_failed_error', { label, error: errorText(e) }));
    }
  }

  function copyTranslation() {
    const text = record?.lines
      .map((l) => l.translated)
      .filter(Boolean)
      .join('\n');
    if (text) void copyText(text, 'common:translated_text');
  }
</script>

<div class="flex flex-wrap items-center gap-2 border-b border-border px-4 py-3 text-xs text-ink-faint">
  <span class="rounded-full bg-surface-muted px-2 py-0.5">{kindLabel(entry.kind)}</span>
  <time datetime={new Date(entry.created_at * 1000).toISOString()}>{dateLabel(entry.created_at)}</time>
  {#if record}
    <span class="min-w-0 break-words rounded-full bg-surface-muted px-2 py-0.5">{t('subtitle-record-detail:audio_source', { source: record.source })}</span>
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
  <ol class="scrollbar-minimal flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-4" aria-label={t('subtitle-record-detail:subtitle_lines')} style:--tc={long ? '7.5rem' : '5rem'}>
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
      <p class="text-sm text-ink-muted">{t('common:loading')}</p>
    {:else}
      {@render plain()}
    {/if}
  </div>
{/if}

<div class="flex flex-wrap items-center gap-2 border-t border-border px-4 py-2">
  {#if record}
    {#if progress}
      <span class="text-sm tabular-nums text-ink-muted" role="status">{t('subtitle-record-detail:retranslating_done_total', { done: progress.done, total: progress.total })}</span>
      <button class="sbtn" onclick={cancel}>{t('common:cancel')}</button>
    {:else}
      <button class="sbtn icon neutral" title={t('subtitle-record-detail:retranslate')} aria-label={t('subtitle-record-detail:retranslate')} onclick={start}>
        <Languages class="h-3.5 w-3.5" />
      </button>
    {/if}
    <button class="sbtn icon neutral" title={t('subtitle-record-detail:export_srt')} aria-label={t('subtitle-record-detail:export_srt')} onclick={() => save('srt')}>
      <Captions class="h-3.5 w-3.5" />
    </button>
    <button class="sbtn icon neutral" title={t('subtitle-record-detail:export_text')} aria-label={t('subtitle-record-detail:export_text')} onclick={() => save('txt')}>
      <FileText class="h-3.5 w-3.5" />
    </button>
    <select
      class="rounded-ml-sm border border-border bg-surface px-2 py-1.5 text-sm text-ink"
      title={t('subtitle-record-detail:export_content')}
      aria-label={t('subtitle-record-detail:export_content')}
      bind:value={content}
    >
      {#each CONTENT_LABELS as [value, label] (value)}
        <option {value}>{t(label)}</option>
      {/each}
    </select>
    <button class="sbtn icon neutral" title={t('common:copy_translation')} aria-label={t('common:copy_translation')} onclick={copyTranslation}>
      <Copy class="h-3.5 w-3.5" />
    </button>
  {/if}
  <button
    class="sbtn icon ml-auto"
    title={t('common:delete_this_item')}
    aria-label={t('common:delete_this_item')}
    onclick={onremove}
  >
    <Trash2 class="h-3.5 w-3.5" />
  </button>
</div>
