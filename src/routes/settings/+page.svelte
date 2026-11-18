<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import { beforeNavigate } from '$app/navigation';
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

  // Keep the index in step with what is on screen.
  function onScroll() {
    let id = visible[0]?.section.id;
    for (const { section } of visible) {
      const el = document.getElementById(`section-${section.id}`);
      if (el && el.getBoundingClientRect().top <= 140) id = section.id;
    }
    if (id) current = id;
  }

  onMount(async () => {
    window.addEventListener('scroll', onScroll, { passive: true });
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
      }
    } catch (e) {
      loadError = String(e);
    }
  });

  // Leaving would silently drop the edits; ask for a decision in the save
  // bar instead (WebView dialogs are not reliable inside Tauri).
  onDestroy(() => {
    window.removeEventListener('scroll', onScroll);
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

  function jump(id: string) {
    current = id;
    document.getElementById(`section-${id}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }
</script>

<div class="mx-auto flex max-w-5xl flex-col gap-4 px-4 pb-28 pt-6 sm:px-6">
  <header class="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
    <div>
      <h1 class="text-xl font-semibold tracking-tight">設定</h1>
      <p class="text-sm text-ink-muted">保存時に即時反映。</p>
    </div>
    <div class="relative w-full sm:w-64">
      <Search class="pointer-events-none absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-ink-faint" />
      <input
        bind:value={query}
        type="search"
        placeholder="設定を検索"
        aria-label="設定を検索"
        class="w-full rounded-ml-sm border border-border bg-surface-elevated py-1.5 pl-8 pr-3 text-sm placeholder:text-ink-faint focus:border-accent/60 focus:outline-none"
      />
    </div>
  </header>

  {#if loadError}
    <p class="rounded-ml border border-red-500/50 bg-red-500/10 p-3 text-sm break-words text-red-500">
      設定の読込に失敗：{loadError}
    </p>
  {:else if !draft || !defaults}
    <p class="text-ink-faint">読込中…</p>
  {:else}
    <div class="flex flex-col gap-4 md:flex-row md:items-start md:gap-8">
      <!-- Section index: a sidebar on wide windows, a scrollable row on narrow ones. -->
      <nav
        aria-label="設定の項目"
        class="scrollbar-minimal -mx-4 flex gap-1 overflow-x-auto px-4 md:sticky md:top-16 md:mx-0 md:w-44 md:shrink-0 md:flex-col md:overflow-visible md:px-0"
      >
        {#each visible as { section } (section.id)}
          <button
            class="shrink-0 rounded-ml-sm px-3 py-1.5 text-left text-sm transition {current === section.id
              ? 'bg-accent-soft text-ink'
              : 'text-ink-muted hover:bg-surface-muted hover:text-ink'}"
            onclick={() => jump(section.id)}
          >
            {section.title}
          </button>
        {/each}
      </nav>

      <div class="flex min-w-0 flex-1 flex-col gap-6">
        {#each visible as { section, fields } (section.id)}
          <section id="section-{section.id}" class="scroll-mt-16">
            <h2 class="text-base font-semibold">{section.title}</h2>
            {#if section.description}
              <p class="mt-0.5 text-sm text-ink-muted">{section.description}</p>
            {/if}
            {#if 'component' in section && section.component === 'data'}
              <div class="mt-3"><DataLocations /></div>
            {:else if 'component' in section}
              <div class="mt-3 flex flex-col gap-3">
                <LocalModels />
                <OcrModels />
                <AsrModels />
              </div>
            {:else}
              <div
                class="mt-3 divide-y divide-border rounded-ml border border-border bg-surface-elevated px-4 shadow-ml"
              >
                {#each fields as field (field.path)}
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
                      value={getPath(draft, field.path)}
                      defaultValue={getPath(defaults, field.path) ?? null}
                      invalid={clashes.includes(field.path)}
                      onChange={(v) => update(field.path, v)}
                    />
                  {/if}
                {/each}
              </div>
            {/if}
          </section>
        {:else}
          <p class="rounded-ml border border-dashed border-border p-6 text-center text-sm text-ink-muted">
            「{query}」：該当なし
          </p>
        {/each}
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
