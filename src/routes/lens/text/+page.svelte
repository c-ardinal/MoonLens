<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { emitTo, type UnlistenFn } from '@tauri-apps/api/event';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import LensText from '$lib/components/LensText.svelte';
  import {
    TEXT_COMMAND_EVENT,
    TEXT_READY_EVENT,
    TEXT_VIEW_EVENT,
    type TextCommand,
    type TextView
  } from '$lib/lens-text';

  // ウィンドウ翻訳のテキスト表示（ウィンドウ外）。内容は本体ウィンドウが送信し、
  // 位置は本体の移動・サイズ変更へバックエンドが追従（lens_text.rs）。

  const params = new URLSearchParams(
    typeof window !== 'undefined' ? window.location.search : ''
  );
  /** 本体ウィンドウの ID（ラベル）。 */
  const id = params.get('id') ?? '';

  let view = $state<TextView>({
    blocks: [],
    translation: '',
    source: null,
    explanation: null,
    placement: 'right'
  });
  const unlisteners: (UnlistenFn | undefined)[] = [];

  function send(command: TextCommand) {
    emitTo(id, TEXT_COMMAND_EVENT, command).catch(() => {});
  }

  function startResize(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    getCurrentWebviewWindow().startResizeDragging('SouthEast').catch(() => {});
  }

  onMount(async () => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') send({ kind: 'close' });
    };
    window.addEventListener('keydown', onKey);
    unlisteners.push(() => window.removeEventListener('keydown', onKey));
    try {
      unlisteners.push(
        await getCurrentWebviewWindow().listen<TextView>(TEXT_VIEW_EVENT, (e) => (view = e.payload))
      );
      await emitTo(id, TEXT_READY_EVENT);
    } catch {
      /* not running inside Tauri */
    }
  });

  onDestroy(() => unlisteners.forEach((u) => u?.()));
</script>

<main class="win">
  <LensText
    {view}
    drag={view.placement === 'free'}
    onplacement={(value) => send({ kind: 'placement', value })}
    onclose={() => send({ kind: 'close' })}
  />
  <div class="grip" role="presentation" title="サイズ変更" onpointerdown={startResize}></div>
</main>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    background: rgb(24, 24, 28);
  }
  .win {
    position: relative;
    box-sizing: border-box;
    height: 100vh;
    overflow: hidden;
    /* 本体の範囲枠と同色：対応関係の明示 */
    box-shadow: inset 0 0 0 1px rgba(139, 92, 246, 0.6);
  }
  .grip {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 14px;
    height: 14px;
    cursor: nwse-resize;
    background: linear-gradient(135deg, transparent 50%, rgba(139, 92, 246, 0.85) 50%);
  }
</style>
