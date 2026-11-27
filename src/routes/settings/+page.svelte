<script lang="ts">
  import PixelTitle from '$lib/components/PixelTitle.svelte';
  import { slant } from '$lib/pixel/slant';
  import { onDestroy, onMount, tick, untrack } from 'svelte';
  import { beforeNavigate } from '$app/navigation';
  import { page } from '$app/state';
  import { Search } from 'lucide-svelte';
  import {
    getDefaultSettings,
    getSettings,
    onSettingsChanged,
    setSettings,
    type Settings
  } from '$lib/ipc';
  import {
    GROUPS,
    SECTIONS,
    changedPaths,
    duplicateHotkeys,
    heldHotkeys,
    fieldMatches,
    getPath,
    savable,
    saveDelay,
    setPath
  } from '$lib/settings-meta';
  import { createAutosave } from '$lib/autosave';
  import { errorText } from '$lib/errors';
  import SettingField from '$lib/components/SettingField.svelte';
  import SettingRow from '$lib/components/SettingRow.svelte';
  import { provideRowSelection } from '$lib/settings-rows.svelte';
  import LocalModels from '$lib/components/LocalModels.svelte';
  import AsrModels from '$lib/components/AsrModels.svelte';
  import OcrModels from '$lib/components/OcrModels.svelte';
  import DataLocations from '$lib/components/DataLocations.svelte';
  import Plugins from '$lib/components/Plugins.svelte';
  import LayoutSets from '$lib/components/LayoutSets.svelte';
  import GlossaryEditor from '$lib/components/GlossaryEditor.svelte';
  import { createRemovalLog, isBlankEntry, type GlossaryEntry } from '$lib/glossary';
  import { close as closeToast, undoable } from '$lib/toast.svelte';
  import InferenceDevice from '$lib/components/InferenceDevice.svelte';
  import { replay, wheelStep, type Effect } from '$lib/pixel/transition';

  /** The selector remembers the language last picked in its toolbar. */
  const SELECTOR_LANG_KEY = 'moonlens.selector.targetLanguage';

  const DEVICE_HELP =
    '翻訳・音声認識の実行先。文字認識（OCR）は CPU 専用。実行結果の詳細はログに記録。';

  /** Bundled defaults, for each field's reset button. */
  let defaults = $state<Settings | null>(null);
  let unlisten: (() => void) | undefined;
  let saved = $state<Settings | null>(null);
  let draft = $state<Settings | null>(null);
  let query = $state('');
  /** 保存状況（見出し横の表示）。'saved' は一定時間後に 'idle' へ復帰。 */
  let status = $state<'idle' | 'saving' | 'saved'>('idle');
  let statusTimer: ReturnType<typeof setTimeout> | undefined;
  let saveError = $state<string | null>(null);
  let loadError = $state<string | null>(null);
  let current = $state(SECTIONS[0].id);

  /** Shortcuts bound twice: only one of each pair could be registered. */
  const clashes = $derived(saved && draft ? heldHotkeys(saved, draft) : []);
  /** 「重複」表示の対象：保存対象外の項目と、その重複相手。 */
  const marked = $derived(draft ? [...new Set([...clashes, ...duplicateHotkeys(draft)])] : []);
  /** 直近の保存開始以降に編集した項目（保存中に元の値へ復帰させた項目を含む）。 */
  let edited = new Set<string>();
  const visible = $derived(
    SECTIONS.map((s) => ({
      section: s,
      fields: 'fields' in s ? s.fields.filter((f) => fieldMatches(f, s, query)) : []
    })).filter(
      ({ section, fields }) =>
        fields.length > 0 ||
        ('component' in section &&
          (!query.trim() || `${section.title} ${section.description ?? ''}`.toLowerCase().includes(query.trim().toLowerCase())))
    )
  );

  onMount(async () => {
    try {
      [defaults, saved] = await Promise.all([getDefaultSettings(), getSettings()]);
      draft = structuredClone($state.snapshot(saved));
      // Settings can change while this screen is open (a model picked
      // below, config.toml edited by hand, this screen's own save). Take
      // the new values and keep the edits not saved yet on top, so the
      // next save never reverts them.
      unlisten = await onSettingsChanged((next) => {
        // 保存中に元の値へ復帰させた項目は saved との差分に現れない：edited で補完
        const paths = new Set([...changedPaths(saved, draft), ...edited]);
        const edits = [...paths].map((p) => [p, getPath(draft, p)] as const);
        saved = next;
        let d = structuredClone(next);
        for (const [p, v] of edits) d = setPath(d, p, v);
        draft = d;
      });
      // Links such as /settings/#section-models land on that section.
      const target = location.hash.replace(/^#section-/, '');
      if (target) {
        await tick();
        jump(target);
      } else {
        jump(SECTIONS[0].id);
      }
    } catch (e) {
      loadError = String(e);
    }
  });

  onDestroy(() => {
    // 画面破棄後の「取消」は保存済み設定との同期なしで上書き：通知を閉鎖
    for (const id of glossaryUndo) closeToast(id, 'dismiss');
    unlisten?.();
    clearTimeout(statusTimer);
    void autosave.flush();
  });

  // 待機中の保存（スライダー・用語集）は画面離脱前に実行
  beforeNavigate(() => {
    void autosave.flush();
  });

  const autosave = createAutosave(save);

  function update(path: string, value: any) {
    draft = setPath(draft, path, value);
    edited.add(path);
    saveError = null;
    autosave.schedule(saveDelay(path));
  }

  /** 表示中の「取消」付き通知（用語集の行削除）。 */
  const glossaryUndo = new Set<number>();
  /** 削除済みの行の復元位置（複数行の削除・取消の順序に非依存）。 */
  const glossaryRemovals = createRemovalLog();

  /**
   * 用語集の行削除。即時に削除・保存し、通知の「取消」で元の位置へ再挿入。
   * 未入力の行は通知なし（喪失する内容なし）。
   */
  function removeGlossaryRow(path: string, index: number) {
    const current: GlossaryEntry[] = $state.snapshot(getPath(draft, path)) ?? [];
    if (index >= current.length) return;
    const { list, row } = glossaryRemovals.remove(current, index);
    update(path, list);
    if (isBlankEntry(row.entry)) return;
    const id = undoable({
      text: '用語 1 件：削除完了',
      failure: '復元失敗',
      undo: async () => {
        glossaryUndo.delete(id);
        update(path, glossaryRemovals.restore($state.snapshot(getPath(draft, path)) ?? [], row));
      }
    });
    glossaryUndo.add(id);
  }

  /**
   * 変更の即時保存。重複中のショートカットは対象外（解消後に保存）。
   * 編集中の値（draft）は保持：保存中の追加入力・用語集の空行を維持。
   */
  async function save() {
    if (!saved || !draft) return;
    edited = new Set();
    const next = savable($state.snapshot(saved), $state.snapshot(draft));
    const changed = changedPaths(saved, next);
    if (changed.length === 0) return;
    clearTimeout(statusTimer);
    status = 'saving';
    try {
      await setSettings(next);
      saved = next;
      if (changed.includes('subtitle.target_language')) {
        // A language picked in the selector toolbar would otherwise keep
        // overriding the one chosen here.
        try {
          localStorage.removeItem(SELECTOR_LANG_KEY);
        } catch {
          /* storage unavailable */
        }
      }
      status = 'saved';
      statusTimer = setTimeout(() => (status = 'idle'), 2000);
    } catch (e) {
      // Saved, but e.g. a shortcut could not be registered.
      status = 'idle';
      saveError = errorText(e);
      try {
        saved = await getSettings();
      } catch {
        /* keep the previous snapshot */
      }
    }
  }

  /** 表示中の小分類（検索結果に含まれない場合は先頭）。 */
  const shown = $derived(visible.find((v) => v.section.id === current) ?? visible[0]);
  /** 大分類：検索結果を含む分類のみ。 */
  const groups = $derived(
    GROUPS.map((g) => ({ group: g, sections: visible.filter((v) => g.sections.includes(v.section.id)) })).filter(
      (g) => g.sections.length > 0
    )
  );
  const shownGroup = $derived(groups.find((g) => g.sections.some((v) => v === shown)) ?? groups[0]);

  /** 選択中の行（選択帯と説明欄の対象）。 */
  const rows = provideRowSelection();
  const MODEL_COMPONENTS = ['llm-translate', 'llm-assist', 'ocr', 'asr'];
  const component = $derived(shown && 'component' in shown.section ? shown.section.component : null);
  let modelsMounted = $state(false);
  $effect(() => {
    if (component && MODEL_COMPONENTS.includes(component)) modelsMounted = true;
  });
  // 項目名の再掲は冗長：説明のない項目は小分類の説明を表示
  const explain = $derived(rows.help ?? shown?.section.description ?? '');

  /** 先頭行を選択状態へ（分類切替時）。 */
  async function selectFirstRow() {
    rows.clear();
    await tick();
    const first = [...document.querySelectorAll<HTMLElement>('.cfg .scroll [data-row]')].find(
      (r) => r.offsetParent !== null
    );
    first?.dispatchEvent(new MouseEvent('mouseenter'));
  }

  /** 小分類 id・大分類 id のどちらでも該当画面へ切替。 */
  function jump(id: string) {
    const group = GROUPS.find((g) => g.id === id);
    const target = group
      ? (groups.find((g) => g.group.id === id)?.sections[0]?.section.id ?? group.sections[0])
      : id;
    if (target === current && rows.active) return;
    // 分類切替時は先頭から表示（前の分類のスクロール位置を持越さない）
    document.querySelector('.cfg .scroll')?.scrollTo({ top: 0 });
    current = target;
    selectFirstRow();
  }

  // 表示中の設定画面へ /settings/#section-xxx で再遷移した場合も該当分類へ切替
  // （選択ツールバーのモデル異常からの導線等。onMount は再実行されないため）
  $effect(() => {
    const target = page.url.hash.replace(/^#section-/, '');
    // 依存はハッシュのみ（編集のたびに選択項目が先頭へ戻る事態を回避）
    if (target && untrack(() => draft) && SECTIONS.some((x) => x.id === target)) untrack(() => jump(target));
  });

  /** 枠内の項目入替の方向：分類・タブの並び順（後方は右から、前方は左から）。枠・タブは固定。 */
  const ORDER = GROUPS.flatMap((g) => g.sections);
  const sectionOrder = (id: string) => ORDER.indexOf(id);
  const sectionMotion = (a: string, b: string): Effect => (sectionOrder(b) > sectionOrder(a) ? 'slide-left' : 'slide-right');

  function moveRow(delta: number) {
    const list = [...document.querySelectorAll<HTMLElement>('.cfg .scroll [data-row]')].filter(
      (r) => r.offsetParent !== null
    );
    if (list.length === 0) return;
    const i = list.findIndex((r) => r.dataset.row === rows.active);
    const row = list[(i + delta + list.length) % list.length];
    row.dispatchEvent(new MouseEvent('mouseenter'));
    row.scrollIntoView({ block: 'nearest' });
    row.querySelector<HTMLElement>('input,select,button')?.focus({ preventScroll: true });
  }

  function cycle<T>(list: T[], at: number, delta: number): T {
    return list[(at + delta + list.length) % list.length];
  }

  /** ホイール：分類一覧の上では大分類、タブ列の上では小分類を 1 個ずつ切替（端で停止）。 */
  function wheelGroup(delta: 1 | -1) {
    const ids = groups.map((g) => g.group.id);
    const next = ids[ids.indexOf(shownGroup?.group.id ?? '') + delta];
    if (next) jump(next);
  }
  function wheelSection(delta: 1 | -1) {
    const ids = shownGroup?.sections.map((v) => v.section.id) ?? [];
    const next = ids[ids.indexOf(shown?.section.id ?? '') + delta];
    if (next) jump(next);
  }

  function onKey(e: KeyboardEvent) {
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    const t = e.target as HTMLElement | null;
    const typing = !!t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName));
    const gids = groups.map((g) => g.group.id);
    const gat = Math.max(0, gids.indexOf(shownGroup?.group.id ?? ''));
    const sids = shownGroup?.sections.map((v) => v.section.id) ?? [];
    const sat = Math.max(0, sids.indexOf(shown?.section.id ?? ''));
    if (e.key === 'PageDown') jump(cycle(gids, gat, 1));
    else if (e.key === 'PageUp') jump(cycle(gids, gat, -1));
    else if (typing) return;
    else if (e.key === 'ArrowRight' && sids.length > 1) jump(cycle(sids, sat, 1));
    else if (e.key === 'ArrowLeft' && sids.length > 1) jump(cycle(sids, sat, -1));
    else if (e.key === 'ArrowDown') moveRow(1);
    else if (e.key === 'ArrowUp') moveRow(-1);
    else if (e.key === '/') document.getElementById('settings-search')?.focus();
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="cfg px-body" class:with-bar={clashes.length > 0 || !!saveError}>
  <header class="head">
    <div class="page-title"><PixelTitle text="設定" tag="h1" height={40} fit /></div>
    <p class="status" class:done={status === 'saved'} role="status">
      {status === 'saving' ? '保存中…' : status === 'saved' ? '保存完了' : '変更は即時保存'}
    </p>
    <!-- 操作案内：全文を収容不可の幅では非表示（途中切れを防止） -->
    <p class="hint" data-grid-glyph data-overflow-exempt><span>↑↓項目 ←→タブ PgUp/Dn分類</span></p>
    <div class="search">
      <Search class="icon" />
      <input id="settings-search" bind:value={query} type="search" placeholder="設定を検索" aria-label="設定を検索" />
    </div>
  </header>

  {#if loadError}
    <p class="px-win error">設定の読込に失敗：{loadError}</p>
  {:else if !draft || !defaults}
    <p class="loading">読込中…</p>
  {:else if visible.length === 0}
    <p class="px-win empty">「{query}」：該当なし</p>
  {:else}
    <div class="body">
      <!-- 大分類：等幅の斜め帯を階段状に配置（左右端とも同じ傾きで整列） -->
      <nav aria-label="設定の分類" class="tabs scrollbar-minimal" style="--n: {groups.length}" use:wheelStep={wheelGroup}>
        {#each groups as { group }, i (group.id)}
          <button
            class="px-chip tab"
            style="--step: {groups.length - 1 - i}"
            data-tone={shownGroup?.group.id === group.id ? 'bone' : undefined}
            aria-current={shownGroup?.group.id === group.id ? 'true' : undefined}
            use:slant
            onclick={() => jump(group.id)}
          >
            {group.title}
          </button>
        {/each}
      </nav>

      <div class="right">
      <section
        class="main"
        id="section-{shown.section.id}"
        aria-label={shown.section.title}
      >
        <div class="frame" aria-hidden="true"></div>
        {#if (shownGroup?.sections.length ?? 0) > 1}
          <!-- 小分類：枠上部の矩形タブ -->
          <div class="subtabs" role="tablist" aria-label="{shownGroup.group.title}の小分類" use:wheelStep={wheelSection}>
            {#each shownGroup.sections as { section } (section.id)}
              <button
                class="subtab"
                role="tab"
                aria-selected={shown.section.id === section.id}
                onclick={() => jump(section.id)}
              >
                {section.title}
              </button>
            {/each}
          </div>
        {/if}
        <div class="scroll scrollbar-minimal">
          <div use:replay={{ key: shown.section.id, effect: sectionMotion }}>
          <!-- モデル管理：一度表示後は分類切替でも保持（取得中の進捗・完了通知を継続） -->
          {#if modelsMounted}
            <div style:display={component === 'llm-translate' ? null : 'none'}><LocalModels role="translate" /></div>
            <div style:display={component === 'llm-assist' ? null : 'none'}><LocalModels role="assistant" /></div>
            <div style:display={component === 'ocr' ? null : 'none'}><OcrModels /></div>
            <div style:display={component === 'asr' ? null : 'none'}><AsrModels /></div>
          {/if}
          {#if component === 'data'}
            <DataLocations />
          {:else if component === 'plugins'}
            <Plugins />
          {:else if component === 'layout-sets'}
            <LayoutSets />
          {:else if !component}
            {#each shown.fields as field (field.path)}
              <SettingRow id={field.path} help={field.help}>
                {#if field.kind === 'glossary'}
                  <GlossaryEditor
                    label={field.label}
                    value={getPath(draft, field.path)}
                    defaultLang={getPath(draft, 'subtitle.target_language') ?? 'ja'}
                    onChange={(v) => update(field.path, v)}
                    onRemove={(i) => removeGlossaryRow(field.path, i)}
                  />
                {:else}
                  <SettingField
                    {field}
                    compact
                    value={getPath(draft, field.path)}
                    defaultValue={getPath(defaults, field.path) ?? null}
                    invalid={marked.includes(field.path)}
                    onChange={(v) => update(field.path, v)}
                  />
                {/if}
              </SettingRow>
              {#if field.path === 'performance.use_gpu_if_available'}
                <SettingRow id="info:inference-device" help={DEVICE_HELP}>
                  <InferenceDevice useGpu={!!getPath(draft, field.path)} />
                </SettingRow>
              {/if}
            {/each}
          {/if}
          </div>
        </div>
      </section>
      <!-- 説明欄：右列の下部（タブ列は全高を使用し、分類の見切れを防止） -->
      <section class="px-win desc" aria-live="polite">
        <span class="px-chip px-win-title" data-tone="amber" use:slant>説明</span>
        <p use:replay={{ key: explain, effect: 'type' }}>
          <!-- 文と参照先の間隔は余白トークンで指定（空白文字の幅は文字サイズ設定により 4px 格子から逸脱） -->
          {explain}{#if rows.link}<a href={rows.link.href} target="_blank" rel="noreferrer">{rows.link.label}</a>{/if}
        </p>
      </section>
      </div>
    </div>
  {/if}
</div>

{#if clashes.length > 0 || saveError}
  <!-- 通知バー：保存不可の変更・保存時のエラー。画面下端固定。本文と同じ左右余白で整列 -->
  <div class="savebar px-zoom">
    <p class="msg error" role="alert">
      {#if clashes.length > 0}ショートカット重複：該当項目は未保存。重複解消後に自動保存。{/if}
      {#if clashes.length > 0 && saveError}<br />{/if}
      {#if saveError}{saveError}{/if}
    </p>
    {#if saveError}
      <button class="px-btn" onclick={() => (saveError = null)}>閉じる</button>
    {/if}
  </div>
{/if}

<style>
  .cfg {
    position: relative;
    display: grid;
    /* 列幅は画面幅で確定（見出し行の内容幅による列の拡張＝横スクロールを防止） */
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: auto minmax(0, 1fr);
    gap: var(--ml-flow);
    padding: var(--ml-page-y) var(--ml-page-x);
    box-sizing: border-box;
  }
  /* 通知バー（画面下端固定）表示中：説明欄との重複を回避 */
  .cfg.with-bar {
    padding-bottom: calc(var(--ml-sp-16) + var(--ml-page-y));
  }
  .head {
    display: flex;
    gap: var(--ml-sp-4);
  }
  .search {
    position: relative;
    flex-shrink: 0;
    width: clamp(240px, 22vw, 300px);
  }
  .search :global(.icon) {
    position: absolute;
    left: var(--ml-sp-3);
    top: 50%;
    width: 24px;
    height: var(--ml-sp-6);
    transform: translateY(-50%);
    color: var(--ml-px-faint);
  }
  .search input {
    width: 100%;
    height: var(--ml-sp-11);
    padding: 0 var(--ml-sp-3) 0 var(--ml-sp-11);
    background: var(--ml-px-black);
    border: 2px solid var(--ml-px-dim);
    color: var(--ml-px-cream);
  }
  .search input:focus {
    border-color: var(--ml-px-amber);
    outline: none;
  }
  .body {
    display: grid;
    grid-template-columns: var(--tab-col) minmax(0, 1fr);
    gap: var(--ml-sp-4);
    min-height: 0;
    /* 大分類の帯：幅 280px、1 段ごとに 12px 後退（帯自体の傾き 0.3 と同等） */
    --tab-w: 280px;
    --tab-step: var(--ml-sp-3);
    --tab-col: calc(var(--tab-w) + 6 * var(--tab-step));
  }
  .tabs {
    display: flex;
    flex-direction: column;
    gap: var(--ml-row-pad);
    overflow-x: hidden;
    overflow-y: auto;
    padding: var(--ml-sp-1) 0;
  }
  .tab {
    flex-shrink: 0;
    width: var(--tab-w);
    margin-left: calc((var(--step) + 6 - (var(--n) - 1)) * var(--tab-step));
    white-space: nowrap;
    text-align: left;
  }
  .tab:not([data-tone]):hover {
    background: var(--ml-px-dim);
    color: var(--ml-px-cream);
  }
  .right {
    display: grid;
    grid-template-rows: minmax(0, 1fr) auto;
    gap: var(--ml-sp-3);
    min-height: 0;
  }
  .main {
    position: relative;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-height: 0;
    /* 枠線（.frame）の領域を確保：内部の余白を枠線の内側から計測 */
    border: 2px solid transparent;
  }
  .frame {
    position: absolute;
    inset: -2px;
    background: var(--ml-px-black);
    border-top: 2px solid var(--ml-px-bone);
    border-left: 2px solid var(--ml-px-bone);
    border-right: 2px solid var(--ml-px-dim);
    border-bottom: 2px solid var(--ml-px-dim);
  }
  .subtabs {
    position: relative;
    grid-row: 1;
    display: flex;
    flex-wrap: wrap;
    gap: var(--ml-sp-1);
    padding: var(--ml-sp-3) var(--ml-sp-7) 0;
    border-bottom: 2px solid var(--ml-px-dim);
  }
  .subtab {
    padding: 0 var(--ml-sp-4);
    line-height: var(--ml-sp-10);
    color: var(--ml-px-mid);
    white-space: nowrap;
  }
  .subtab:hover {
    background: var(--ml-px-dim);
    color: var(--ml-px-cream);
  }
  .subtab[aria-selected='true'] {
    background: var(--ml-px-amber);
    color: var(--ml-px-black);
  }
  .scroll {
    position: relative;
    grid-row: 2;
    height: 100%;
    overflow-x: hidden;
    overflow-y: auto;
    padding: var(--ml-sp-3) var(--ml-sp-7) var(--ml-sp-4);
    box-sizing: border-box;
  }
  .desc a {
    margin-left: var(--ml-sp-2);
    color: var(--ml-px-mid);
    text-decoration: underline;
  }
  .desc {
    /* 見出し札（上へ 20px 突出）分の余白 */
    margin: var(--ml-sp-5) 0 0;
    padding-top: var(--ml-sp-6);
    box-sizing: border-box;
  }
  /* 説明文：常時 2 行分の高さ（項目選択でのウィンドウの伸縮を防止）。超過分はウィンドウ内スクロール */
  .desc p {
    height: calc(var(--ml-sp-9) * 2);
    margin: 0;
    overflow-y: auto;
  }
  .status {
    flex-shrink: 0;
    margin: 0;
    color: var(--ml-px-faint);
    white-space: nowrap;
  }
  .status.done {
    color: var(--ml-px-amber);
  }
  .hint {
    display: flex;
    flex: 1 1 0;
    flex-wrap: wrap;
    justify-content: flex-end;
    min-width: 0;
    height: var(--ml-sp-9);
    margin: 0;
    overflow: hidden;
    color: var(--ml-px-faint);
    white-space: nowrap;
  }
  /* 幅 0 の先頭項目：案内文が 1 行目へ収容不可の場合に 2 行目（表示範囲外）へ折返し */
  .hint::before {
    content: '';
    height: 100%;
  }
  .savebar {
    position: fixed;
    inset: auto 0 0 0;
    z-index: 40;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--ml-sp-4);
    min-height: var(--ml-sp-16);
    padding: var(--ml-sp-2) var(--ml-page-x);
    box-sizing: border-box;
    background: var(--ml-px-black);
    border-top: 2px solid var(--ml-px-amber);
  }
  .savebar .msg {
    min-width: 0;
    flex: 1;
    color: var(--ml-px-mid);
    overflow-wrap: anywhere;
  }
  .savebar .error {
    color: var(--ml-px-red);
  }
  .px-btn {
    flex-shrink: 0;
    height: var(--ml-sp-12);
    padding: 0 var(--ml-sp-4);
    border: 2px solid var(--ml-px-dim);
    background: var(--ml-px-black);
    color: var(--ml-px-mid);
  }
  .px-btn:hover {
    color: var(--ml-px-cream);
    border-color: var(--ml-px-faint);
  }
  /* 枠内の部品（モデル管理・データ）：外枠と二重化する独自の外枠を解除 */
  .scroll :global(section.rounded-ml.border) {
    padding: 0 0 var(--ml-sp-5);
    border: 0;
    border-bottom: 2px dotted var(--ml-px-dim);
    border-radius: 0;
    background: none;
    box-shadow: none;
  }
  .scroll :global(section.rounded-ml.border:last-child) {
    padding-bottom: 0;
    border-bottom: 0;
  }
  .loading,
  .error,
  .empty {
    margin-left: var(--ml-sp-7);
  }
  .error {
    color: var(--ml-px-red);
  }
</style>

