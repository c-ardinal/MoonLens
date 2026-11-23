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
  import { SECTIONS, changedPaths, duplicateHotkeys, fieldMatches, getPath, setPath } from '$lib/settings-meta';
  import { errorText } from '$lib/errors';
  import SettingField from '$lib/components/SettingField.svelte';
  import LocalModels from '$lib/components/LocalModels.svelte';
  import AsrModels from '$lib/components/AsrModels.svelte';
  import OcrModels from '$lib/components/OcrModels.svelte';
  import DataLocations from '$lib/components/DataLocations.svelte';
  import GlossaryEditor from '$lib/components/GlossaryEditor.svelte';
  import { compactGlossary } from '$lib/glossary';

  /** The selector remembers the language last picked in its toolbar. */
  const SELECTOR_LANG_KEY = 'moonlens.selector.targetLanguage';

  /** Bundled defaults, for each field's reset button. */
  let defaults = $state<Settings | null>(null);
  let unlisten: (() => void) | undefined;
  let saved = $state<Settings | null>(null);
  let draft = $state<Settings | null>(null);
  let query = $state('');
  let saving = $state(false);
  let message = $state<{ kind: 'ok' | 'error'; text: string } | null>(null);
  let loadError = $state<string | null>(null);
  let current = $state(SECTIONS[0].id);

  const changed = $derived(saved && draft ? changedPaths(saved, draft) : []);
  /** Shortcuts bound twice: only one of each pair could be registered. */
  const clashes = $derived(draft ? duplicateHotkeys(draft) : []);
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
      // below, config.toml edited by hand). Take the new values and keep
      // the user's unsaved edits on top, so saving never reverts them.
      unlisten = await onSettingsChanged((next) => {
        const edits = changed.map((p) => [p, getPath(draft, p)] as const);
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

  // Leaving would silently drop the edits; ask for a decision in the save
  // bar instead (WebView dialogs are not reliable inside Tauri).
  onDestroy(() => {
    unlisten?.();
  });

  beforeNavigate((nav) => {
    if (changed.length > 0 && nav.type !== 'leave') {
      nav.cancel();
      message = { kind: 'error', text: '未保存の変更あり。「保存」か「破棄」を選択。' };
    }
  });

  function update(path: string, value: any) {
    draft = setPath(draft, path, value);
    message = null;
  }

  async function save() {
    if (!draft || saving || clashes.length > 0) return;
    saving = true;
    message = null;
    let next = $state.snapshot(draft);
    // Rows added but never filled in are not worth keeping.
    next = setPath(next, 'translation.glossary', compactGlossary(getPath(next, 'translation.glossary')));
    const languageChanged = changed.includes('subtitle.target_language');
    try {
      await setSettings(next);
      saved = structuredClone(next);
      draft = structuredClone(next);
      if (languageChanged) {
        // A language picked in the selector toolbar would otherwise keep
        // overriding the one chosen here.
        try {
          localStorage.removeItem(SELECTOR_LANG_KEY);
        } catch {
          /* storage unavailable */
        }
      }
      message = { kind: 'ok', text: '保存完了' };
    } catch (e) {
      // Saved, but e.g. a shortcut could not be registered.
      message = { kind: 'error', text: errorText(e) };
      try {
        saved = await getSettings();
      } catch {
        /* keep the previous snapshot */
      }
    } finally {
      saving = false;
    }
  }

  function revert() {
    if (saved) draft = structuredClone($state.snapshot(saved));
    message = null;
  }

  /** 表示中の分類（検索結果に含まれない場合は先頭）。 */
  const shown = $derived(visible.find((v) => v.section.id === current) ?? visible[0]);
  /** 選択中の項目（選択帯と説明窓の対象）。 */
  let activePath = $state<string | null>(null);
  const activeField = $derived(shown?.fields.find((f) => f.path === activePath) ?? null);
  const showingModels = $derived(
    !!shown && 'component' in shown.section && shown.section.component !== 'data'
  );
  let modelsMounted = $state(false);
  $effect(() => {
    if (showingModels) modelsMounted = true;
  });
  // 項目名の再掲は冗長：説明のない項目は分類の説明を表示
  const explain = $derived(activeField?.help ?? shown?.section.description ?? activeField?.label ?? '');

  function jump(id: string) {
    current = id;
    const s = visible.find((v) => v.section.id === id);
    activePath = s?.fields[0]?.path ?? null;
  }

  // 表示中の設定画面へ /settings/#section-xxx で再遷移した場合も該当分類へ切替
  // （選択ツールバーのモデル異常からの導線等。onMount は再実行されないため）
  $effect(() => {
    const target = page.url.hash.replace(/^#section-/, '');
    // 依存はハッシュのみ（編集のたびに選択項目が先頭へ戻る事態を回避）
    if (target && untrack(() => draft) && SECTIONS.some((x) => x.id === target)) untrack(() => jump(target));
  });

  function moveRow(delta: number) {
    const fields = shown?.fields ?? [];
    if (fields.length === 0) return;
    const i = fields.findIndex((f) => f.path === activePath);
    const next = fields[(i + delta + fields.length) % fields.length];
    activePath = next.path;
    const row = document.querySelector<HTMLElement>(`[data-row="${next.path}"]`);
    row?.scrollIntoView({ block: 'nearest' });
    row?.querySelector<HTMLElement>('input,select,button')?.focus({ preventScroll: true });
  }

  function onKey(e: KeyboardEvent) {
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    const t = e.target as HTMLElement | null;
    const typing = !!t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName));
    const ids = visible.map((v) => v.section.id);
    const at = Math.max(0, ids.indexOf(shown?.section.id ?? ''));
    if (e.key === 'PageDown') jump(ids[(at + 1) % ids.length]);
    else if (e.key === 'PageUp') jump(ids[(at - 1 + ids.length) % ids.length]);
    else if (typing) return;
    else if (e.key === 'ArrowDown') moveRow(1);
    else if (e.key === 'ArrowUp') moveRow(-1);
    else if (e.key === '/') document.getElementById('settings-search')?.focus();
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="cfg" class:with-bar={changed.length > 0 || !!message}>
  <header class="head">
    <div class="page-title"><PixelTitle text="設定" tag="h1" height={40} fit /></div>
    <p class="hint">↑↓項目 PgUp/Dn分類 /検索</p>
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
      <!-- 分類：斜めの階段状タブ（下段ほど左へ後退） -->
      <nav aria-label="設定の分類" class="tabs scrollbar-minimal">
        {#each visible as { section }, i (section.id)}
          <button
            class="px-chip tab"
            style="--step: {visible.length - 1 - i}"
            data-tone={shown?.section.id === section.id ? 'bone' : undefined}
            aria-current={shown?.section.id === section.id ? 'true' : undefined}
            use:slant
            onclick={() => jump(section.id)}
          >
            {section.title}
          </button>
        {/each}
      </nav>

      <div class="right">
      <section class="main" id="section-{shown.section.id}" aria-label={shown.section.title}>
        <div class="frame" aria-hidden="true"></div>
        <div class="scroll scrollbar-minimal">
          <!-- モデル管理：一度表示後は分類切替でも保持（取得中の進捗・完了通知を継続） -->
          {#if modelsMounted}
            <div class="flex flex-col gap-6" style:display={showingModels ? null : "none"}>
              <LocalModels />
              <OcrModels />
              <AsrModels />
            </div>
          {/if}
          {#if 'component' in shown.section && shown.section.component === 'data'}
            <DataLocations />
          {:else if 'component' in shown.section}
            <!-- モデル管理は上部で描画 -->
          {:else}
            {#each shown.fields as field (field.path)}
              <div
                class="row"
                class:active={activePath === field.path}
                data-row={field.path}
                role="presentation"
                onmouseenter={() => (activePath = field.path)}
                onfocusin={() => (activePath = field.path)}
              >
                {#if activePath === field.path}
                  <div class="band outer" use:slant aria-hidden="true"></div>
                  <div class="band inner" use:slant aria-hidden="true"></div>
                {/if}
                {#if field.kind === 'glossary'}
                  <GlossaryEditor
                    label={field.label}
                    help={field.help}
                    value={getPath(draft, field.path)}
                    defaultLang={getPath(draft, 'subtitle.target_language') ?? 'ja'}
                    onChange={(v) => update(field.path, v)}
                  />
                {:else}
                  <SettingField
                    {field}
                    compact
                    value={getPath(draft, field.path)}
                    defaultValue={getPath(defaults, field.path) ?? null}
                    invalid={clashes.includes(field.path)}
                    onChange={(v) => update(field.path, v)}
                  />
                {/if}
              </div>
            {/each}
          {/if}
        </div>
      </section>
      <!-- 説明窓：右列の下部（タブ列は全高を使用し、分類の見切れを防止） -->
      <section class="px-win desc" aria-live="polite">
        <span class="px-chip px-win-title" data-tone="amber" use:slant>説明</span>
        <p>{explain}</p>
      </section>
      </div>
    </div>
  {/if}
</div>

{#if changed.length > 0 || message}
  <div class="fixed inset-x-0 bottom-0 z-40 border-t border-border bg-surface-elevated/95 backdrop-blur-xl">
    <div class="mx-auto flex max-w-5xl flex-wrap items-center justify-between gap-2 px-4 py-3 sm:px-6">
      <p
        class="min-w-0 flex-1 text-sm break-words {message?.kind === 'error'
          ? 'text-red-500'
          : 'text-ink-muted'}"
        role="status"
      >
        {#if message && changed.length === 0}
          {message.text}
        {:else}
          未保存の変更：{changed.length} 件
          {#if clashes.length > 0}<br /><span class="text-red-500">ショートカット重複あり。重複の解消が必要</span>{/if}
          {#if message?.kind === 'error'}<br />{message.text}{/if}
        {/if}
      </p>
      {#if changed.length > 0}
        <div class="flex shrink-0 gap-2">
          <button
            class="rounded-ml-sm border border-border bg-surface px-3 py-1.5 text-sm text-ink-muted hover:text-ink"
            onclick={revert}
          >
            破棄
          </button>
          <button
            class="rounded-ml-sm bg-accent px-4 py-1.5 text-sm font-medium text-white disabled:opacity-40"
            onclick={save}
            disabled={saving || clashes.length > 0}
          >
            {saving ? '保存中…' : '保存'}
          </button>
        </div>
      {:else}
        <button class="shrink-0 text-sm text-ink-faint hover:text-ink" onclick={() => (message = null)}>閉じる</button>
      {/if}
    </div>
  </div>
{/if}

<style>
  .cfg {
    position: relative;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 12px;
    height: calc(100vh - var(--ml-nav-h, 64px));
    padding: 12px 28px 8px 40px;
    box-sizing: border-box;
  }
  /* 保存バー（画面下端固定）表示中：説明窓との重複を回避 */
  .cfg.with-bar {
    padding-bottom: 76px;
  }
  .head {
    display: flex;
    align-items: flex-start;
    gap: 16px;
  }
  .search {
    position: relative;
    margin-top: 20px;
    flex-shrink: 0;
    width: clamp(240px, 22vw, 300px);
  }
  .search :global(.icon) {
    position: absolute;
    left: 10px;
    top: 50%;
    width: 24px;
    height: 24px;
    transform: translateY(-50%);
    color: var(--ml-px-faint);
  }
  .search input {
    width: 100%;
    padding: 2px 10px 2px 44px;
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
    grid-template-columns: 340px minmax(0, 1fr);
    gap: 16px;
    min-height: 0;
  }
  .tabs {
    display: flex;
    flex-direction: column;
    gap: 0;
    overflow-x: hidden;
    overflow-y: auto;
    padding: 4px 0;
  }
  .tab {
    margin-left: calc(var(--step) * 6px);
    margin-right: 16px;
    white-space: nowrap;
    line-height: 34px;
    text-align: left;
  }
  .tab[data-tone] {
    margin-right: -8px;
  }
  .tab:not([data-tone]):hover {
    background: var(--ml-px-dim);
    color: var(--ml-px-cream);
  }
  .right {
    display: grid;
    grid-template-rows: minmax(0, 1fr) auto;
    gap: 12px;
    min-height: 0;
  }
  .main {
    position: relative;
    min-height: 0;
  }
  .frame {
    position: absolute;
    inset: 0 0 0 20px;
    background: var(--ml-px-black);
    border-top: 2px solid var(--ml-px-bone);
    border-left: 2px solid var(--ml-px-bone);
    border-right: 2px solid var(--ml-px-dim);
    border-bottom: 2px solid var(--ml-px-dim);
  }
  .scroll {
    position: relative;
    height: 100%;
    overflow-x: hidden;
    overflow-y: auto;
    padding: 16px 28px 16px 60px;
    box-sizing: border-box;
  }
  .row {
    position: relative;
    border-bottom: 2px dotted var(--ml-px-dim);
  }
  .row:last-child {
    border-bottom: 0;
  }
  .band {
    position: absolute;
    pointer-events: none;
  }
  .band.outer {
    /* 枠（左 22px・右 2px の内側）へ収める：枠線との重複なし */
    inset: -6px -24px -4px -36px;
    background: var(--ml-px-bone);
  }
  .band.inner {
    inset: -2px -20px 0 -30px;
    background: var(--ml-px-amber);
  }
  .row > :global(:not(.band)) {
    position: relative;
  }
  .row.active :global(label),
  .row.active :global(.value-text) {
    color: var(--ml-px-black);
  }
  /* 琥珀の帯上のスライダー：つまみ・溝を黒系へ反転 */
  .row.active :global(input[type='range']::-webkit-slider-thumb) {
    background: var(--ml-px-black);
    box-shadow: none;
  }
  .row.active :global(input[type='range']::-webkit-slider-runnable-track) {
    background: var(--ml-px-faint);
  }
  .desc {
    /* 見出し札（上へ 20px 突出）がタブ列と重ならない余白 */
    margin: 20px 0 0 20px;
    padding-top: 26px;
    box-sizing: border-box;
  }
  .hint {
    min-width: 0;
    margin: 24px 0 0 auto;
    overflow: hidden;
    line-height: 44px;
    color: var(--ml-px-faint);
    white-space: nowrap;
  }
  .loading,
  .error,
  .empty {
    margin-left: 28px;
  }
  .error {
    color: var(--ml-px-red);
  }
</style>

