<script lang="ts" module>

  export interface ChoiceRow {
    id: string;
    label: string;
    size: number;
    note: string;
    installed: boolean;
    downloading?: boolean;
    partial_bytes?: number;
    license?: string;
    license_url?: string;
    /** 取得前に同意が必要なライセンス id。 */
    consent?: string | null;
  }
</script>

<script lang="ts">
  import { t } from '$lib/i18n';
  import { Trash2 } from 'lucide-svelte';
  import { formatBytes, replacementCandidates } from '$lib/model-setup';
  import LicenseConsent from './LicenseConsent.svelte';
  import SettingRow from './SettingRow.svelte';

  interface Props {
    /** 行 id の接頭辞（例：llm:translate）。 */
    key: string;
    /** 機能名（例：翻訳）。案内文に使用。 */
    feature: string;
    rows: ChoiceRow[];
    /** 選択中モデル（常に 1 件）。 */
    selected: string;
    recommended?: string;
    /** 取得中モデルの進捗（0〜1）。 */
    progress?: Record<string, number | undefined>;
    busy?: Record<string, boolean>;
    /** 他機能で使用中の場合、その機能名（削除不可）。 */
    usedElsewhere?: (id: string) => string | null;
    /** 削除ボタンの表示（OCR は削除非対応）。 */
    removable?: boolean;
    onselect: (id: string) => Promise<void>;
    /** 取得。select：取得完了後に選択切替。 */
    oninstall: (id: string, select: boolean) => Promise<void>;
    oncancel?: (id: string) => void;
    /** 削除。replacement：先に切替える代替モデル（未取得なら取得後に切替）。 */
    onremove?: (id: string, replacement: string | null) => Promise<void>;
    /** 同意済みのライセンス id。 */
    accepted?: string[];
    /** ライセンスへの同意の保存後（一覧の再取得用）。 */
    onaccepted?: (license: string) => void | Promise<void>;
  }
  let {
    key,
    feature,
    rows,
    selected,
    recommended = '',
    progress = {},
    busy = {},
    usedElsewhere = () => null,
    removable = true,
    onselect,
    oninstall,
    oncancel,
    onremove,
    accepted = [],
    onaccepted
  }: Props = $props();

  /** 未取得モデルの選択：取得確認中のモデル。 */
  let asking = $state<string | null>(null);
  /** 削除確認中（未使用モデル）。 */
  let confirmDelete = $state<string | null>(null);
  /** 使用中モデルの削除：代替モデル選択中。 */
  let replacing = $state<string | null>(null);
  let replacement = $state('');
  /** 取得前の許諾確認中：表示する行・ライセンス・同意後に続行する元の操作。 */
  let consenting = $state<{ at: string; license: string; proceed: () => Promise<void> } | null>(null);
  /** 削除不可の理由（他機能で使用中）。 */
  let blocked = $state<{ id: string; text: string } | null>(null);

  const loading = (m: ChoiceRow) => !!busy[m.id] || !!m.downloading || progress[m.id] !== undefined;
  const help = (m: ChoiceRow) =>
    t('{note}。', { note: t(m.note) }) +
    (m.id === recommended ? t('本 PC の構成を基準とした推奨モデル。') : '') +
    (m.installed ? '' : t('未取得。'));

  /** 確認欄の表示時：一覧の枠内へ全体を表示。 */
  function reveal(node: HTMLElement) {
    requestAnimationFrame(() => node.scrollIntoView?.({ block: 'nearest' }));
  }

  function reset() {
    asking = null;
    confirmDelete = null;
    replacing = null;
    blocked = null;
    consenting = null;
  }

  /**
   * 取得を伴う操作の実行。未同意のライセンスが有れば `at` 行へ許諾確認を表示し、
   * 同意の保存後に元の操作を続行。
   */
  async function gate(at: string, target: ChoiceRow | undefined, proceed: () => Promise<void>) {
    const license = target && !target.installed ? target.consent : null;
    if (license && !accepted.includes(license)) {
      reset();
      consenting = { at, license, proceed };
    } else await proceed();
  }

  async function agreed() {
    const c = consenting;
    if (!c) return;
    await onaccepted?.(c.license);
    await c.proceed();
  }

  async function choose(m: ChoiceRow) {
    if (m.id === selected) return;
    reset();
    if (m.installed) await onselect(m.id);
    else if (!loading(m)) asking = m.id;
  }

  const install = (id: string, select: boolean) =>
    gate(id, rows.find((r) => r.id === id), async () => {
      reset();
      await oninstall(id, select);
    });

  function askDelete(m: ChoiceRow) {
    reset();
    const other = usedElsewhere(m.id);
    if (other) {
      blocked = { id: m.id, text: t('{other}で使用中。{other}の選択変更後に削除可能', { other }) };
      return;
    }
    if (m.id === selected) {
      replacing = m.id;
      replacement = replacementCandidates(rows, m.id, recommended)[0]?.id ?? '';
    } else {
      confirmDelete = m.id;
    }
  }

  /** 削除。代替モデルの取得を伴う場合は、そのライセンスの同意を先に確認。 */
  const remove = (id: string, repl: string | null) =>
    gate(id, rows.find((r) => r.id === repl), async () => {
      reset();
      await onremove?.(id, repl);
    });
</script>

<div role="radiogroup" aria-label={t('{feature}モデル', { feature })}>
  {#each rows as m (m.id)}
    {@const p = progress[m.id]}
    {@const checked = m.id === selected}
    <SettingRow
      id="{key}:{m.id}"
      help={help(m)}
      link={m.license_url ? { href: m.license_url, label: t('ライセンス：{license}', { license: m.license ?? '' }) } : null}
    >
      <div class="srow">
        <button class="spick" role="radio" aria-checked={checked} onclick={() => choose(m)}>
          <span class="sradio" aria-hidden="true"></span>
          <span class="row-label">{m.label}</span>
          {#if m.id === recommended}<span class="stag rec">{t('推奨')}</span>{/if}
        </button>
        <span class="row-ctl">
          <span class="row-value">{p !== undefined ? `${Math.round(p * 100)}%` : formatBytes(m.size)}</span>
          {#if loading(m)}
            {#if oncancel}<button class="sbtn" onclick={() => oncancel(m.id)}>{t('中止')}</button>{/if}
          {:else if !m.installed}
            <span class="stag off">{t('未取得')}</span>
          {:else if removable && onremove}
            {#if confirmDelete === m.id}
              <button class="sbtn danger" onclick={() => remove(m.id, null)} onblur={() => (confirmDelete = null)}>
                {t('削除確定')}
              </button>
            {:else}
              <button class="sbtn icon" title={t('削除')} aria-label={t('{label} を削除', { label: m.label })} onclick={() => askDelete(m)}>
                <Trash2 class="h-4 w-4" />
              </button>
            {/if}
          {/if}
        </span>
      </div>

      {#if asking === m.id}
        <div use:reveal class="sprompt" role="group" aria-label={t('{label} の取得確認', { label: m.label })}>
          <p>{t('未取得。取得後に切替（{size}）', { size: formatBytes(m.size - (m.partial_bytes ?? 0)) })}</p>
          <button class="sbtn on" onclick={() => install(m.id, true)}>{t('取得・切替')}</button>
          <button class="sbtn" onclick={() => (asking = null)}>{t('中止')}</button>
        </div>
      {:else if checked && !m.installed && !loading(m)}
        <div class="sprompt warn" role="alert">
          <p>{t('選択中モデル未取得。{feature}に必要', { feature })}</p>
          <button class="sbtn on" onclick={() => install(m.id, false)}>
            {(m.partial_bytes ?? 0) > 0 ? t('取得再開') : t('取得')}
          </button>
        </div>
      {/if}

      {#if consenting?.at === m.id}
        <div use:reveal class="sprompt col" role="group" aria-label={t('利用許諾の確認')}>
          <LicenseConsent
            license={consenting.license}
            onaccepted={agreed}
            oncancel={() => (consenting = null)}
          />
        </div>
      {/if}

      {#if blocked?.id === m.id}
        <div class="sprompt warn" role="alert">
          <p>{blocked.text}</p>
          <button class="sbtn" onclick={() => (blocked = null)}>{t('閉じる')}</button>
        </div>
      {/if}

      {#if replacing === m.id}
        <div use:reveal class="sprompt col" role="group" aria-label={t('代替モデルの選択')}>
          <p>{t('使用中モデル。削除前に代替モデル選択')}</p>
          <div class="salts" role="radiogroup" aria-label={t('代替モデル')}>
            {#each replacementCandidates(rows, m.id, recommended) as alt (alt.id)}
              <button
                class="spick alt"
                role="radio"
                aria-checked={replacement === alt.id}
                onclick={() => (replacement = alt.id)}
              >
                <span class="sradio" aria-hidden="true"></span>
                <span>{alt.label}</span>
                {#if alt.id === recommended}<span class="stag rec">{t('推奨')}</span>{/if}
                <span class="row-value">{alt.installed ? t('取得済') : t('{size} 取得', { size: formatBytes(alt.size) })}</span>
              </button>
            {/each}
          </div>
          <div class="sacts">
            <button class="sbtn danger" disabled={!replacement} onclick={() => remove(m.id, replacement)}>
              {t('切替・削除')}
            </button>
            <button class="sbtn" onclick={() => (replacing = null)}>{t('中止')}</button>
          </div>
        </div>
      {/if}

      {#if p !== undefined}
        <div class="sprogress" aria-hidden="true"><span style="width: {(p * 100).toFixed(1)}%"></span></div>
      {/if}
    </SettingRow>
  {/each}
</div>
