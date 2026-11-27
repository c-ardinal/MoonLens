<script lang="ts">
  import { acceptLicense } from '$lib/ipc';
  import { errorText } from '$lib/errors';
  import { LICENSES } from '$lib/licenses';

  interface Props {
    /** ライセンス id（`CatalogModel.consent`）。 */
    license: string;
    agreeLabel?: string;
    cancelLabel?: string;
    /** 同意の保存後に実行（元の操作の続行）。 */
    onaccepted: () => void | Promise<void>;
    oncancel: () => void;
  }
  let { license, agreeLabel = '同意・取得', cancelLabel = '中止', onaccepted, oncancel }: Props = $props();

  const info = $derived(LICENSES[license]);
  let checked = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);

  async function agree() {
    busy = true;
    error = null;
    try {
      await acceptLicense(license);
    } catch (e) {
      error = errorText(e);
      busy = false;
      return;
    }
    busy = false;
    await onaccepted();
  }
</script>

{#if info}
  <div class="lc">
    <p class="lead">{info.lead}</p>
    <p class="cap">要点（要約）</p>
    <ul>
      {#each info.points as p (p)}<li>{p}</li>{/each}
    </ul>
    <p class="links">
      {#each [info.fullText, ...info.links] as l (l.href)}
        <a href={l.href} target="_blank" rel="noreferrer">{l.label}</a>
      {/each}
    </p>
    {#if error}<p class="err" role="alert">{error}</p>{/if}
    <div class="acts">
      <label class="agree">
        <input type="checkbox" bind:checked />
        {info.fullText.label}を確認、内容へ同意
      </label>
      <button class="sbtn on" disabled={!checked || busy} onclick={agree}>{agreeLabel}</button>
      <button class="sbtn" disabled={busy} onclick={oncancel}>{cancelLabel}</button>
    </div>
  </div>
{/if}

<style>
  .lc {
    display: flex;
    flex-direction: column;
    gap: var(--ml-sp-2);
  }
  .lc p,
  .lc ul {
    margin: 0;
  }
  .cap {
    font-weight: bold;
  }
  ul {
    padding-left: var(--ml-sp-5);
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: var(--ml-sp-1) var(--ml-sp-5);
  }
  .links a {
    color: inherit;
    text-decoration: underline;
  }
  .agree {
    display: inline-flex;
    align-items: center;
    gap: var(--ml-sp-2);
    margin-right: auto;
    cursor: pointer;
  }
  .agree input {
    width: 24px;
    height: 24px;
    accent-color: currentColor;
  }
  .err {
    color: var(--ml-px-red);
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--ml-sp-2);
  }
</style>
