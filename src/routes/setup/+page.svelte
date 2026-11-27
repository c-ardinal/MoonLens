<script lang="ts">
  import { t } from '$lib/i18n';
  import { onDestroy, onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import PixelTitle from '$lib/components/PixelTitle.svelte';
  import LicenseConsent from '$lib/components/LicenseConsent.svelte';
  import { LICENSES } from '$lib/licenses';
  import { slant } from '$lib/pixel/slant';
  import {
    cancelAsrDownload,
    cancelLlmDownload,
    completeSetup,
    installAsrModel,
    installLlmModel,
    installOcrModels,
    listAsrModels,
    listLlmModels,
    ocrModelsStatus,
    onAsrProgress,
    onLlmProgress,
    onModelProgress,
    selectAsrModel,
    selectLlmModel
  } from '$lib/ipc';
  import { systemProfile } from '$lib/system-profile.svelte';
  import {
    buildFeatures,
    downloadQueue,
    FEATURE_TITLES,
    formatBytes,
    hardwareLines,
    pendingConsents,
    totalBytes,
    type DownloadItem,
    type FeaturePlan
  } from '$lib/model-setup';
  import { errorText } from '$lib/errors';

  type Step = 'check' | 'select' | 'consent' | 'download';
  type ItemState = 'wait' | 'run' | 'done' | 'error' | 'cancel';

  /** title は日本語の原文（表示時に `t()` で翻訳）。 */
  const STEPS: { id: Step; title: string }[] = [
    { id: 'check', title: '構成診断' },
    { id: 'select', title: 'モデル選定' },
    { id: 'consent', title: '許諾確認' },
    { id: 'download', title: '一括取得' }
  ];

  let step = $state<Step>('check');
  let loadError = $state<string | null>(null);
  let features = $state<FeaturePlan[]>([]);
  /** 同意済みのライセンス id（取得前の許諾確認の要否判定）。 */
  let accepted = $state<string[]>([]);
  let queue = $state<DownloadItem[]>([]);
  let states = $state<Record<string, ItemState>>({});
  let fractions = $state<Record<string, number>>({});
  let errors = $state<Record<string, string>>({});
  let running = $state(false);
  let current = $state<DownloadItem | null>(null);
  const unlisteners: (() => void)[] = [];

  const report = $derived(systemProfile.report);
  const lines = $derived(report ? hardwareLines(report) : []);
  const planned = $derived(downloadQueue(features));
  const pending = $derived(pendingConsents(planned, accepted));
  const total = $derived(totalBytes(planned));
  const free = $derived(report?.profile.disk_free ?? null);
  /** 取得後も 1 GB の余裕を確保（バックエンドの容量検査と同等）。 */
  const short = $derived(free != null && total + 1_073_741_824 > free);
  const finished = $derived(queue.length > 0 && queue.every((d) => states[keyOf(d)] === 'done'));
  const failed = $derived(queue.some((d) => ['error', 'cancel'].includes(states[keyOf(d)])));

  const keyOf = (d: DownloadItem) => `${d.kind}:${d.id}`;

  async function load() {
    loadError = null;
    try {
      const [llm, asr, ocr] = await Promise.all([listLlmModels(), listAsrModels(), ocrModelsStatus()]);
      const r = await systemProfile.reload();
      accepted = llm.accepted_licenses;
      features = buildFeatures(llm, asr, ocr, r?.recommended ?? null);
    } catch (e) {
      loadError = errorText(e);
    }
  }

  onMount(async () => {
    load();
    const set = (k: string, f: number) => (fractions = { ...fractions, [k]: f });
    unlisteners.push(await onLlmProgress((p) => set(`llm:${p.id}`, p.fraction)));
    unlisteners.push(await onAsrProgress((p) => set(`asr:${p.id}`, p.fraction)));
    unlisteners.push(await onModelProgress((p) => set('ocr:ppocr', p.fraction ?? 0)));
  });
  onDestroy(() => unlisteners.forEach((u) => u()));

  async function fetchOne(d: DownloadItem) {
    if (d.kind === 'llm') await installLlmModel(d.id);
    else if (d.kind === 'asr') await installAsrModel(d.id);
    else await installOcrModels();
  }

  /** 選択内容を設定へ反映（取得済みの選択のみ。未取得の機能は既存選択を維持）。 */
  async function applySelection(done: Set<string>) {
    for (const f of features) {
      if (!f.enabled || f.feature === 'ocr') continue;
      const c = f.choices.find((x) => x.id === f.selected);
      const kind = f.feature === 'asr' ? 'asr' : 'llm';
      if (!c || !(c.installed || done.has(`${kind}:${c.id}`))) continue;
      if (f.feature === 'asr') await selectAsrModel(c.id);
      else await selectLlmModel(c.id, f.feature === 'assistant' ? 'assistant' : 'translate');
    }
  }

  /** 承認後の一括取得。失敗・中止分は再試行で続行。 */
  async function run() {
    if (step !== 'download') {
      queue = planned;
      states = Object.fromEntries(queue.map((d) => [keyOf(d), 'wait' as ItemState]));
      step = 'download';
    }
    running = true;
    const done = new Set(queue.filter((d) => states[keyOf(d)] === 'done').map(keyOf));
    for (const d of queue) {
      const k = keyOf(d);
      if (states[k] === 'done') continue;
      current = d;
      states = { ...states, [k]: 'run' };
      try {
        await fetchOne(d);
        states = { ...states, [k]: 'done' };
        done.add(k);
      } catch (e) {
        const cancelled = String(e).includes('cancelled');
        states = { ...states, [k]: cancelled ? 'cancel' : 'error' };
        if (!cancelled) errors = { ...errors, [k]: errorText(e) };
        if (cancelled) break;
      }
    }
    current = null;
    try {
      await applySelection(done);
    } catch (e) {
      errors = { ...errors, apply: errorText(e) };
    }
    running = false;
  }

  /** モデル選定の確定：同意が必要なライセンスが有れば許諾確認、無ければ取得開始。 */
  function approve() {
    if (pending.length) step = 'consent';
    else run();
  }

  /** 同意の保存後：全ライセンスの同意が揃った時点で一括取得。 */
  function onAccepted(id: string) {
    accepted = [...accepted, id];
    if (!pending.length) run();
  }

  /** モデル選定へ移行：空き容量を再取得（診断後の容量変化を反映）。 */
  async function toSelect() {
    step = 'select';
    await systemProfile.reload();
  }

  function cancel() {
    const d = current;
    if (!d) return;
    if (d.kind === 'llm') cancelLlmDownload(d.id);
    else if (d.kind === 'asr') cancelAsrDownload(d.id);
  }

  async function finish() {
    await completeSetup().catch(() => {});
    goto('/');
  }

  const stateText = (d: DownloadItem) => {
    const s = states[keyOf(d)];
    if (s === 'run') return `${Math.round((fractions[keyOf(d)] ?? 0) * 100)}%`;
    return t({ wait: '待機', done: '完了', error: '失敗', cancel: '中止' }[s ?? 'wait'] ?? '');
  };
</script>

<div class="setup">
  <header class="head">
    <div class="page-title"><PixelTitle text={t('初期設定')} tag="h1" height={40} fit /></div>
    <ol class="steps" aria-label={t('手順')}>
      {#each STEPS as s, i (s.id)}
        <li aria-current={step === s.id ? 'step' : undefined}>{i + 1} {t(s.title)}</li>
      {/each}
    </ol>
  </header>

  <main class="body scrollbar-minimal">
    {#if loadError}
      <p class="px-win error" role="alert">{t('読込失敗：{loadError}', { loadError })}</p>
    {:else if step === 'check'}
      <section class="px-win panel" aria-labelledby="hw-title">
        <span class="px-win-title px-chip" data-tone="amber" id="hw-title" use:slant>{t('本 PC の構成')}</span>
        {#if !report}
          <p class="muted">{t('診断中…')}</p>
        {:else}
          <dl class="hw">
            {#each lines as l (l.label)}
              <dt>{l.label}</dt>
              <dd>{l.value}</dd>
            {/each}
          </dl>
        {/if}
      </section>
      <p class="lead">
        {t('本 PC の構成から機能別の推奨モデルを選定。モデルは本 PC 内で動作し、診断結果・翻訳内容の外部送信は皆無。')}
      </p>
    {:else if step === 'select'}
      <p class="lead">{t('各機能のモデルを選択。初期選択は本 PC の推奨モデル。')}</p>
      {#each features as f (f.feature)}
        <section class="px-win panel" aria-labelledby="f-{f.feature}">
          <span class="px-win-title px-chip" data-tone="amber" id="f-{f.feature}" use:slant>{t(f.title)}</span>
          <div class="fhead">
            <p class="muted">{f.reason ? t('推奨理由：{reason}', { reason: t(f.reason) }) : ''}</p>
            {#if !f.required}
              <label class="opt">
                <input type="checkbox" bind:checked={f.enabled} />
                {t('今回取得')}
              </label>
            {/if}
          </div>
          <div role="radiogroup" aria-labelledby="f-{f.feature}" class:off={!f.enabled}>
            {#each f.choices as c (c.id)}
              <button
                class="spick choice"
                role="radio"
                aria-checked={f.selected === c.id}
                disabled={!f.enabled}
                onclick={() => (f.selected = c.id)}
              >
                <span class="sradio" aria-hidden="true"></span>
                <span class="name">{c.label}</span>
                {#if c.id === f.recommended}<span class="stag rec">{t('推奨')}</span>{/if}
                <span class="size">{c.installed ? t('取得済') : formatBytes(c.size)}</span>
              </button>
            {/each}
          </div>
          {#if !f.enabled}
            <p class="muted">{t('後日、設定画面のモデル一覧から取得可能。')}</p>
          {/if}
        </section>
      {/each}
    {:else if step === 'consent'}
      {#each pending as id (id)}
        <section class="px-win panel" aria-labelledby="lc-{id}">
          <span class="px-win-title px-chip" data-tone="amber" id="lc-{id}" use:slant>{t('利用許諾の確認')}</span>
          <LicenseConsent
            license={id}
            cancelLabel={t('前画面')}
            onaccepted={() => onAccepted(id)}
            oncancel={() => (step = 'select')}
          />
        </section>
      {/each}
    {:else}
      <section class="px-win panel" aria-labelledby="dl-title">
        <span class="px-win-title px-chip" data-tone="amber" id="dl-title" use:slant>{t('一括取得')}</span>
        {#if queue.length === 0}
          <p>{t('取得対象なし。選択モデルは全件取得済。')}</p>
        {/if}
        <ul class="dl">
          {#each queue as d (keyOf(d))}
            {@const k = keyOf(d)}
            <li data-state={states[k]}>
              <span class="name">{d.label}</span>
              <span class="muted">{d.features.map((x) => t(FEATURE_TITLES[x])).join(t('・'))}</span>
              <span class="size">{formatBytes(d.size)}</span>
              <span class="st">{stateText(d)}</span>
              {#if states[k] === 'run'}
                <span class="pbar" aria-hidden="true"><span style="width: {((fractions[k] ?? 0) * 100).toFixed(1)}%"></span></span>
              {/if}
              {#if errors[k]}<p class="err">{errors[k]}</p>{/if}
            </li>
          {/each}
        </ul>
        {#if errors.apply}<p class="err">{errors.apply}</p>{/if}
      </section>
      {#if !running && (finished || queue.length === 0)}
        <p class="lead" role="status">{t('準備完了。選択モデルで各機能が使用可能。')}</p>
      {/if}
    {/if}
  </main>

  <footer class="actions">
    {#if step === 'check'}
      <button class="px-btn" onclick={finish}>{t('スキップ')}</button>
      <button class="px-btn primary" disabled={!report && !loadError} onclick={toSelect}>{t('次へ')}</button>
    {:else if step === 'select'}
      <p class="sum" class:warn={short}>
        {free != null
          ? t('合計 {total}／残容量 {free}', { total: formatBytes(total), free: formatBytes(free) })
          : t('合計 {total}', { total: formatBytes(total) })}{short ? t('。残容量不足') : ''}
      </p>
      <button class="px-btn" onclick={() => (step = 'check')}>{t('前画面')}</button>
      <button class="px-btn primary" disabled={short} onclick={approve}>
        {planned.length ? t('承認・一括取得') : t('適用')}
      </button>
    {:else if step === 'consent'}
      <p class="sum">{t('{names}：同意後に一括取得を開始', { names: pending.map((id) => (LICENSES[id] ? t(LICENSES[id].title) : id)).join(t('、')) })}</p>
    {:else if running}
      <p class="sum">{t('取得中：{label}', { label: current?.label ?? '' })}</p>
      <button class="px-btn" disabled={current?.kind === 'ocr'} onclick={cancel}>{t('中止')}</button>
    {:else}
      {#if failed}
        <p class="sum warn">{t('一部未完了。再試行または後日設定画面から取得')}</p>
        <button class="px-btn" onclick={finish}>{t('後日取得')}</button>
        <button class="px-btn primary" onclick={run}>{t('再試行')}</button>
      {:else}
        <button class="px-btn primary" onclick={finish}>{t('開始')}</button>
      {/if}
    {/if}
  </footer>
</div>

<style>
  .setup {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    gap: var(--ml-sp-4);
    height: calc(100vh - var(--ml-nav-h, 64px));
    padding: var(--ml-page-y) var(--ml-page-x);
    box-sizing: border-box;
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--ml-sp-6);
    flex-wrap: wrap;
  }
  .steps {
    display: flex;
    gap: var(--ml-sp-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .steps li {
    padding: 0 var(--ml-sp-3);
    border: 2px solid var(--ml-px-dim);
    color: var(--ml-px-mid);
    white-space: nowrap;
  }
  .steps li[aria-current='step'] {
    border-color: var(--ml-px-bone);
    background: var(--ml-px-bone);
    color: var(--ml-px-black);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--ml-sp-9);
    min-height: 0;
    overflow-y: auto;
    padding: var(--ml-sp-6) var(--ml-sp-2) var(--ml-sp-2) var(--ml-sp-1);
  }
  .panel {
    padding-top: var(--ml-sp-7);
  }
  .lead {
    margin: 0;
    color: var(--ml-px-cream);
  }
  .muted {
    margin: 0;
    color: var(--ml-px-mid);
  }
  .hw {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: var(--ml-sp-2) var(--ml-sp-6);
    margin: 0;
  }
  .hw dt {
    color: var(--ml-px-amber);
  }
  .hw dd {
    margin: 0;
    color: var(--ml-px-cream);
    overflow-wrap: anywhere;
  }
  .fhead {
    display: flex;
    align-items: center;
    gap: var(--ml-sp-4);
    margin-bottom: var(--ml-sp-2);
  }
  .fhead .muted {
    flex: 1;
  }
  .opt {
    display: inline-flex;
    align-items: center;
    gap: var(--ml-sp-2);
    color: var(--ml-px-cream);
    white-space: nowrap;
    cursor: pointer;
  }
  .opt input {
    width: 24px;
    height: 24px;
    accent-color: var(--ml-px-amber);
  }
  .choice {
    width: 100%;
    min-height: var(--ml-sp-12);
    border-bottom: 2px dotted var(--ml-px-dim);
  }
  .choice:last-child {
    border-bottom: 0;
  }
  .choice:hover:not(:disabled) .name {
    color: var(--ml-px-amber);
  }
  .choice .name {
    color: var(--ml-px-cream);
  }
  .choice .size {
    margin-left: auto;
    color: var(--ml-px-mid);
    white-space: nowrap;
  }
  .off {
    opacity: 0.4;
  }
  .dl {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .dl li {
    position: relative;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--ml-sp-1) var(--ml-sp-4);
    padding: var(--ml-sp-2) 0;
    border-bottom: 2px dotted var(--ml-px-dim);
  }
  .dl li:last-child {
    border-bottom: 0;
  }
  .dl .name {
    color: var(--ml-px-cream);
  }
  .dl .size {
    margin-left: auto;
    color: var(--ml-px-mid);
  }
  .dl .st {
    min-width: 4em;
    text-align: right;
    color: var(--ml-px-mid);
    font-variant-numeric: tabular-nums;
  }
  .dl li[data-state='run'] .st {
    color: var(--ml-px-amber);
  }
  .dl li[data-state='done'] .st {
    color: var(--ml-px-cream);
  }
  .dl li:is([data-state='error'], [data-state='cancel']) .st,
  .err {
    color: var(--ml-px-red);
  }
  .err {
    flex-basis: 100%;
    margin: 0;
  }
  .pbar {
    position: absolute;
    inset: auto 0 -2px 0;
    height: var(--ml-sp-1);
    background: var(--ml-px-dim);
  }
  .pbar > span {
    display: block;
    height: 100%;
    background: var(--ml-px-cream);
  }
  .actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--ml-sp-2);
    padding-top: var(--ml-sp-2);
    border-top: 2px solid var(--ml-px-dim);
  }
  .sum {
    flex: 1;
    margin: 0;
    color: var(--ml-px-cream);
  }
  .sum.warn {
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
  .px-btn.primary {
    border-color: var(--ml-px-amber);
    background: var(--ml-px-amber);
    color: var(--ml-px-black);
  }
  .px-btn:disabled {
    opacity: 0.4;
  }
</style>
