<script lang="ts">
  import { t } from '$lib/i18n';
  import { tick } from 'svelte';
  import {
    allowsDrag,
    editableOf,
    menuItemsFor,
    runEditAction,
    type MenuItem
  } from '$lib/webview-guard';

  // 全ウィンドウ共通：組込ブラウザの標準メニューを抑止し、編集操作のみの専用メニューを表示。
  let items = $state<MenuItem[]>([]);
  let pos = $state({ x: 0, y: 0 });
  let active = $state(-1);
  let menu = $state<HTMLDivElement | null>(null);
  let editable: HTMLElement | null = null;

  const open = $derived(items.length > 0);

  function close() {
    items = [];
    active = -1;
    editable = null;
  }

  async function onContextMenu(e: MouseEvent) {
    // 開発時のみ：Shift + 右クリックで標準メニュー（要素検証用）。
    if (import.meta.env.DEV && e.shiftKey) return;
    e.preventDefault();
    if (menu?.contains(e.target as Node)) return;
    editable = editableOf(e.target);
    items = menuItemsFor(e.target, window.getSelection()?.toString() ?? '');
    if (!items.length) return close();
    active = -1;
    pos = { x: e.clientX, y: e.clientY };
    await tick();
    if (!menu) return;
    // 画面端：ウィンドウ内へ補正。
    const r = menu.getBoundingClientRect();
    pos = {
      x: Math.max(0, Math.min(pos.x, window.innerWidth - r.width)),
      y: Math.max(0, Math.min(pos.y, window.innerHeight - r.height))
    };
  }

  function onDragStart(e: DragEvent) {
    if (!allowsDrag(e.target)) e.preventDefault();
  }

  async function run(item: MenuItem) {
    if (!item.enabled) return;
    const target = editable;
    close();
    await runEditAction(item.action, target);
  }

  function step(dir: 1 | -1) {
    const n = items.length;
    for (let i = 1; i <= n; i++) {
      const next = (active + dir * i + n * 2) % n;
      if (items[next].enabled) {
        active = next;
        return;
      }
    }
  }

  // フォーカスは編集欄に維持（選択範囲の保持）。キー操作はウィンドウ側で先取。
  function onKeyDown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'Escape') close();
    else if (e.key === 'ArrowDown') step(1);
    else if (e.key === 'ArrowUp') step(active < 0 ? 1 : -1);
    else if (e.key === 'Enter' && active >= 0) run(items[active]);
    else return close();
    e.preventDefault();
    e.stopPropagation();
  }

  function onPointerDown(e: PointerEvent) {
    if (open && !menu?.contains(e.target as Node)) close();
  }
</script>

<svelte:window
  oncontextmenu={onContextMenu}
  ondragstartcapture={onDragStart}
  onkeydowncapture={onKeyDown}
  onpointerdowncapture={onPointerDown}
  onblur={close}
  onresize={close}
  onwheel={close}
/>

{#if open}
  <!-- mousedown の既定動作を抑止：編集欄のフォーカスと選択範囲を維持 -->
  <div
    bind:this={menu}
    class="ctx-menu"
    role="menu"
    tabindex="-1"
    aria-label={t('context-menu:edit_menu')}
    style:left="{pos.x}px"
    style:top="{pos.y}px"
    onmousedown={(e) => e.preventDefault()}
  >
    {#each items as item, i (item.action)}
      <button
        type="button"
        role="menuitem"
        tabindex="-1"
        class:active={i === active}
        disabled={!item.enabled}
        onmouseenter={() => (active = item.enabled ? i : -1)}
        onclick={() => run(item)}
      >
        {item.label}
      </button>
    {/each}
  </div>
{/if}

<style>
  .ctx-menu {
    position: fixed;
    z-index: 1000;
    min-width: 128px;
    padding: 4px;
    display: flex;
    flex-direction: column;
    background: var(--ml-surface-elevated);
    color: var(--ml-ink);
    border: var(--ml-border-width, 1px) solid var(--ml-border);
    border-radius: var(--ml-radius-sm);
    box-shadow: var(--ml-shadow);
    font-family: var(--ml-font-ui);
    font-size: 16px;
    line-height: 24px;
    user-select: none;
    -webkit-user-select: none;
  }
  button {
    padding: 4px 12px;
    text-align: left;
    white-space: nowrap;
    border-radius: var(--ml-radius-sm);
    outline: none;
  }
  button.active {
    background: var(--ml-accent-soft);
  }
  button:disabled {
    color: var(--ml-ink-faint);
    cursor: default;
  }
</style>
