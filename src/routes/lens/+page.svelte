<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { emitTo, type UnlistenFn } from '@tauri-apps/api/event';
  import PrivacyBadge from '$lib/components/PrivacyBadge.svelte';
  import FlowText from '$lib/components/FlowText.svelte';
  import LensActions from '$lib/components/LensActions.svelte';
  import LensText from '$lib/components/LensText.svelte';
  import AttachMenu from '$lib/components/AttachMenu.svelte';
  import {
    ATTACH_EVENT,
    DETACHED,
    targetName,
    type AttachDock,
    type Attachment
  } from '$lib/attach';
  import {
    getSettings,
    fitLens,
    lensControls,
    listLensWindows,
    openLensBeside,
    restoreLens,
    runExplain,
    setLensInteractive,
    setLensOperate,
    setLensLanguage,
    setLensReadMode,
    setLensTextDock,
    windowAttachment,
    type CaptureFlowResult,
    type LensBefore,
    type LensBlock,
    type LensDescriptor,
    type LensReadMode
  } from '$lib/ipc';
  import { joinedText, languageLabel, maskClip, rgb } from '$lib/lens-overlay';
  import {
    addSample,
    observed,
    remainingSeconds,
    subtitleBand,
    type Sample
  } from '$lib/lens-fit';
  import { liveBoxes, liveText, type LensLiveEvent } from '$lib/live';
  import {
    TEXT_COMMAND_EVENT,
    TEXT_READY_EVENT,
    TEXT_VIEW_EVENT,
    loadPlacement,
    savePlacement,
    textWindowLabel,
    type Explanation,
    type TextCommand,
    type TextPlacement,
    type TextView
  } from '$lib/lens-text';
  import { extractText } from '$lib/result';
  import { errorText } from '$lib/errors';
  import { displayAccelerator } from '$lib/hotkey';

  // Window translation, continuous. The window sits over the observed
  // range: the header above it can be dragged, the edges and the corner
  // grip resize it, and the body lets the mouse through to the app
  // underneath (the backend switches that by pointer position). Shows the
  // translations in place over the range, or as text in the text view:
  // a panel inside the window, or a window of its own outside of it.

  const params = new URLSearchParams(
    typeof window !== 'undefined' ? window.location.search : ''
  );
  const id = $derived(params.get('id') ?? '');

  let source = $state<string | null>(null);
  let translation = $state('');
  let viaCloud = $state(false);
  let provider = $state('');
  let busy = $state(false);
  let lastError = $state<string | null>(null);
  /** The last reading found no text in the range. */
  let empty = $state(false);
  /** Read the range continuously and re-translate on change (default). */
  let continuous = $state(true);
  /** Continuous reading: new text is with the model. */
  let translating = $state(false);
  /** Continuous reading: text recognition is running. */
  let reading = $state(false);
  /** Reading or translating: the sweep runs over the range. */
  const working = $derived(busy || reading || translating);
  /** Blocks of the last reading, painted in place over the range. */
  let blocks = $state<LensBlock[]>([]);
  /** Blocks whose source changed or moved away; hidden until re-read. */
  let hidden = $state<Set<number>>(new Set());
  /** Show the original (translations off) / the text view. */
  let showOriginal = $state(false);
  let showText = $state(false);
  /** Show-original hotkey held: every window hides its translations. */
  let peek = $state(false);
  /** Pause hotkey: no window reads or translates, translations hidden
   * (the screen changes unseen, so they would go stale). */
  let paused = $state(false);
  let pauseKey = $state('');
  let explanation = $state<Explanation | null>(null);
  let sourceLanguage = $state<string | null>(null);
  let targetLanguage = $state('ja');
  let readMode = $state<LensReadMode>('subtitle');
  let dpr = $state(typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1);
  let vw = $state(typeof window !== 'undefined' ? window.innerWidth : 0);
  const boxes = $derived(liveBoxes(blocks, hidden, dpr, vw * dpr));
  const hasText = $derived(!!(source?.trim() || translation.trim()));
  let headerEl: HTMLElement;
  const unlisteners: (UnlistenFn | undefined)[] = [];
  let reportedHeaderHeight = 0;

  const win = getCurrentWebviewWindow();

  async function reportInsets() {
    await tick();
    if (!headerEl) return;
    const top = Math.ceil(headerEl.getBoundingClientRect().height);
    if (top === reportedHeaderHeight) return;
    reportedHeaderHeight = top;
    try {
      await invoke('set_lens_insets', { id, top, bottom: 0, left: 0, right: 0 });
    } catch {
      /* Tauri may not be ready in dev */
    }
  }

  /** Bumped when the lens moves or resizes: results of captures started
   * before that belong to another spot and must not be painted. */
  let geometry = 0;
  /** `geometry` when the latest capture started. */
  let captureGeometry = 0;

  function apply(r: CaptureFlowResult, at = captureGeometry) {
    source = r.source_text;
    // No text found: show the empty state, not "(photo detected)".
    translation = 'Intent' in (r.result.output as object) ? '' : extractText(r.result);
    viaCloud = r.result.via_cloud;
    provider = r.result.used_provider;
    sourceLanguage = r.source_language ?? null;
    empty = !source?.trim() && !translation.trim();
    blocks = at === geometry ? (r.blocks ?? []) : [];
    hidden = new Set();
    // QR code etc.: no boxes to place, so it is shown as text.
    if (!blocks.length && translation.trim()) showText = true;
  }

  /** Continuous reading (`moonlens://lens-live`). */
  function applyLive(ev: LensLiveEvent) {
    if (ev.kind === 'blocks') {
      if (fit.kind === 'observing') fit.samples = addSample(fit.samples, ev.blocks);
      const t = liveText(ev.blocks);
      blocks = ev.blocks;
      hidden = new Set();
      source = t.source;
      translation = t.translation;
      translating = ev.translating;
      empty = false;
      lastError = null;
      if (ev.provider) {
        provider = ev.provider;
        viaCloud = ev.via_cloud ?? false;
      }
    } else if (ev.kind === 'reading') {
      reading = ev.on;
    } else if (ev.kind === 'stale') {
      hidden = new Set([...hidden, ...ev.indices]);
    } else if (ev.kind === 'clear') {
      blocks = [];
      hidden = new Set();
      source = null;
      translation = '';
      translating = false;
      empty = true;
      lastError = null;
    } else if (ev.kind === 'decoded') {
      // QR code: no boxes to place, so it is shown as text.
      blocks = [];
      hidden = new Set();
      source = null;
      translation = ev.text;
      translating = false;
      empty = false;
      lastError = null;
      showText = true;
    } else {
      translating = false;
      lastError = errorText(ev.message);
    }
  }

  /** Counts pauses: a one-off capture started before one is discarded. */
  let pauseEpoch = 0;
  /** Resumed while a one-off capture was running: read again after it. */
  let rereadQueued = false;

  function setPaused(on: boolean) {
    if (on && !paused) pauseEpoch += 1;
    paused = on;
    // Resumed: the reader reads at once; what was shown before the pause
    // stays hidden until then.
    if (on) return;
    hidden = new Set(blocks.map((_, i) => i));
    // Without continuous reading nothing reads on its own.
    if (continuous) return;
    if (busy) rereadQueued = true;
    else refresh();
  }

  async function refresh() {
    if (busy || paused) return;
    // Continuous: ask the reader to read now (also when nothing changed).
    if (continuous) {
      try {
        if (await invoke<boolean>('refresh_lens', { id })) return;
      } catch {
        /* fall back to a one-off capture */
      }
    }
    busy = true;
    lastError = null;
    const at = (captureGeometry = geometry);
    const epoch = pauseEpoch;
    try {
      const r = (await invoke('capture_lens_now', { id })) as CaptureFlowResult;
      // Paused meanwhile: the screen changed unseen, the result is stale.
      if (epoch === pauseEpoch) apply(r, at);
    } catch (e) {
      if (epoch === pauseEpoch) lastError = errorText(e);
    } finally {
      busy = false;
    }
    if (rereadQueued) {
      rereadQueued = false;
      refresh();
    }
  }

  async function close() {
    await invoke('close_lens_window', { id });
  }

  function copy() {
    const text = showOriginal ? (source ?? '') : translation;
    if (text) navigator.clipboard.writeText(text).catch(() => {});
  }

  async function changeLanguage(code: string) {
    if (code === targetLanguage) return;
    targetLanguage = code;
    await setLensLanguage(id, code).catch(() => {});
    // One-off mode: nothing re-reads on its own.
    if (!continuous) refresh();
  }

  function changeReadMode(mode: LensReadMode) {
    readMode = mode;
    setLensReadMode(id, mode).catch(() => {});
  }

  async function explain() {
    const src = source?.trim() || joinedText(blocks, 'source');
    if (!src) return;
    showText = true;
    // One request at a time: the model serves them in turn anyway.
    if (explanation?.status === 'pending') return;
    explanation = { status: 'pending' };
    try {
      const r = await runExplain(
        // copy-lint-ignore（LLM プロンプト）
        `次のテキストの内容を${languageLabel(targetLanguage) || '日本語'}で簡潔に説明してください:\n\n${src}`
      );
      explanation = { status: 'done', text: extractText(r) };
    } catch (e) {
      explanation = { status: 'failed', message: errorText(e) };
    }
  }

  $effect(() => {
    invoke('set_lens_continuous', { id, on: continuous }).catch(() => {});
    if (!continuous) {
      translating = false;
      reading = false;
      // No readings to observe any more.
      untrack(() => {
        if (fit.kind !== 'observing') return;
        stopObserving();
        fit = { kind: 'idle' };
        noteFit('常時翻訳 OFF：観測中止');
      });
    }
  });

  /** Where the text view goes: inside the window, or outside of it (a
   * window of its own that follows this one; see `lens_text.rs`). */
  let placement = $state<TextPlacement>(loadPlacement());
  const textInside = $derived(showText && placement === 'inside');
  const textOutside = $derived(showText && placement !== 'inside');
  const textView = $derived<TextView>({
    blocks,
    translation,
    source,
    explanation,
    placement,
    mode: paused ? 'paused' : peek ? 'original' : 'normal'
  });
  /** Bumped when the outside text window reports it is listening. */
  let textReady = $state(0);
  function setPlacement(p: TextPlacement) {
    placement = p;
    savePlacement(p);
  }
  $effect(() => {
    setLensTextDock(id, showText && placement !== 'inside' ? placement : null).catch(() => {});
  });
  $effect(() => {
    void textReady;
    if (!textOutside) return;
    emitTo(textWindowLabel(id), TEXT_VIEW_EVENT, $state.snapshot(textView)).catch(() => {});
  });

  /** The overflow menu of the header is open (it hangs into the body). */
  let menuOpen = $state(false);

  /** The window of another app this one is attached to: it stays on
   * that window as the app moves and resizes it (see `attach.rs`). */
  let attachment = $state<Attachment>(DETACHED);
  let attachOpen = $state(false);
  /** Bumped on a new attachment: the frame flashes once. */
  let snapped = $state(0);
  const ATTACH_DOCKS: { value: AttachDock; label: string; title: string }[] = [
    { value: 'cover', label: '全体', title: '吸着先の全体へ重畳' },
    { value: 'free', label: '相対', title: '現在の位置・サイズを吸着先との相対位置で保持' }
  ];
  function applyAttachment(next: Attachment) {
    if (next.target && next.target.id !== attachment.target?.id) snapped++;
    attachment = next;
  }

  /** Operate mode: the body takes the mouse, so the translation can be
   * selected and copied. Off (default): clicks go to the app underneath.
   * The text panel and the open menus have controls of their own, so the
   * body takes the mouse for them too. */
  let operate = $state(false);
  let operateKey = $state('');
  $effect(() => {
    setLensInteractive(id, operate || textInside || menuOpen || attachOpen).catch(() => {});
  });
  function setOperate(on: boolean) {
    operate = on;
    setLensOperate(id, on).catch(() => {});
    if (!on) window.getSelection()?.removeAllRanges();
  }

  /**
   * Fitting to the subtitle band: observe the readings for a while, then
   * shrink the window to where text appeared (`lens-fit.ts`). The window
   * stays as fitted; the same button puts it back.
   */
  type Fit =
    | { kind: 'idle' }
    | { kind: 'observing'; started: number; elapsed: number; samples: Sample[] }
    | { kind: 'fitted'; before: LensBefore };
  let fit = $state<Fit>({ kind: 'idle' });
  /** Outcome of the last observation, shown for a moment. */
  let fitNote = $state<string | null>(null);
  let fitNoteTimer: ReturnType<typeof setTimeout> | undefined;
  let fitTimer: ReturnType<typeof setInterval> | undefined;
  /** Until this time, moves and resizes are the window being fitted
   * or put back, not the user's. */
  let fitMoveUntil = 0;
  const FIT_MOVE_MS = 1500;

  function noteFit(text: string) {
    fitNote = text;
    clearTimeout(fitNoteTimer);
    fitNoteTimer = setTimeout(() => (fitNote = null), 5000);
  }

  function stopObserving() {
    clearInterval(fitTimer);
    fitTimer = undefined;
  }

  function startFit() {
    // The band comes from the readings of continuous translation.
    continuous = true;
    fitNote = null;
    // Blocks hidden by a move or a change belong to another picture.
    const current = blocks.filter((_, i) => !hidden.has(i));
    fit = { kind: 'observing', started: Date.now(), elapsed: 0, samples: addSample([], current) };
    fitTimer = setInterval(() => {
      if (fit.kind !== 'observing') return stopObserving();
      fit.elapsed = Date.now() - fit.started;
      if (observed(fit.elapsed, fit.samples.length)) finishFit(fit.samples);
    }, 500);
  }

  async function finishFit(samples: Sample[]) {
    stopObserving();
    fit = { kind: 'idle' };
    const area = {
      w: Math.round(window.innerWidth * dpr),
      h: Math.round((window.innerHeight - reportedHeaderHeight) * dpr)
    };
    const band = subtitleBand($state.snapshot(samples) as Sample[], area);
    if (band.kind === 'none') return noteFit('文字未検出：整形中止');
    if (band.kind === 'fits') return noteFit('整形不要：現在の範囲が適正');
    try {
      fitMoveUntil = Date.now() + FIT_MOVE_MS;
      const before = await fitLens(id, band.rect);
      fit = { kind: 'fitted', before };
      noteFit('字幕領域へ整形完了');
    } catch (e) {
      fitMoveUntil = 0;
      lastError = errorText(e);
    }
  }

  function toggleFit() {
    if (fit.kind === 'observing') {
      stopObserving();
      fit = { kind: 'idle' };
      noteFit('観測中止');
    } else if (fit.kind === 'fitted') {
      const { before } = fit;
      fit = { kind: 'idle' };
      fitMoveUntil = Date.now() + FIT_MOVE_MS;
      restoreLens(id, before).catch((e) => (lastError = errorText(e)));
    } else {
      startFit();
    }
  }

  /** The user moved or resized the window: boxes observed so far belong
   * to another range, and a fitted window is the user's own again. */
  function fitInterrupted() {
    if (Date.now() < fitMoveUntil) return;
    if (fit.kind === 'observing') {
      stopObserving();
      noteFit('移動・サイズ変更を検出：観測中止');
    }
    fit = { kind: 'idle' };
  }

  /** Maximized: the window translates the whole screen. */
  let maximized = $state(false);
  function toggleMaximize() {
    win.toggleMaximize().catch(() => {});
  }

  function startResize(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    win.startResizeDragging('SouthEast').catch(() => {});
  }

  /** Last window position / size, to ignore move events that move nothing. */
  let lastPos = '';
  let lastSize = '';
  function moved(key: string, prev: string): boolean {
    if (key === prev) return false;
    geometry++;
    fitInterrupted();
    // Boxes no longer line up: hide them until the range is read again.
    hidden = new Set(blocks.map((_, i) => i));
    // Have the reader look now: it waits for the lens to hold still, and
    // shows the boxes again even if the move turns out to be none.
    if (continuous) invoke('refresh_lens', { id }).catch(() => {});
    return true;
  }

  onMount(async () => {
    reportInsets();
    const onResize = () => (vw = window.innerWidth);
    window.addEventListener('resize', onResize);
    unlisteners.push(() => window.removeEventListener('resize', onResize));
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (attachOpen) attachOpen = false;
      else if (menuOpen) menuOpen = false;
      else close();
    };
    window.addEventListener('keydown', onKey);
    unlisteners.push(() => window.removeEventListener('keydown', onKey));
    // The attach menu closes on a press anywhere else, like the overflow
    // menu, and when the window loses the keyboard. Not on Linux: there
    // the window is hidden for every capture, which also takes the
    // keyboard away, about once a second.
    const hiddenForCapture = /Linux/.test(navigator.userAgent);
    const onPress = (e: PointerEvent) => {
      const el = e.target as Element | null;
      if (attachOpen && !el?.closest?.('.am, [data-action="attach"]')) attachOpen = false;
    };
    const onBlur = () => {
      if (!hiddenForCapture) attachOpen = false;
    };
    window.addEventListener('pointerdown', onPress);
    window.addEventListener('blur', onBlur);
    unlisteners.push(() => {
      window.removeEventListener('pointerdown', onPress);
      window.removeEventListener('blur', onBlur);
    });
    // Language: the one this lens was opened with, else the configured one.
    Promise.all([listLensWindows(), getSettings()])
      .then(([lenses, s]: [LensDescriptor[], any]) => {
        const me = lenses.find((l) => l.id === id);
        targetLanguage = me?.target_language ?? s?.subtitle?.target_language ?? 'ja';
        readMode = me?.read_mode ?? 'subtitle';
        const k = s?.hotkeys?.toggle_window_operate;
        operateKey = k ? displayAccelerator(k) : '';
        const p = s?.hotkeys?.toggle_window_pause;
        pauseKey = p ? displayAccelerator(p) : '';
      })
      .catch(() => {});
    try {
      // Only this window's events: with several windows open, the global
      // `listen` would also paint the other windows' results here.
      // One-off captures (continuous reading off) also arrive here.
      unlisteners.push(
        await win.listen<CaptureFlowResult>('moonlens://lens-result', (e) => apply(e.payload))
      );
      // Pause and show-original hotkeys (all windows at once). A window
      // opened during a pause or a hold starts in that state.
      // An event that arrives before the state asked for here is newer.
      const seen = { pause: false, peek: false };
      unlisteners.push(
        await win.listen<boolean>('moonlens://lens-pause', (e) => {
          seen.pause = true;
          setPaused(e.payload);
        })
      );
      unlisteners.push(
        await win.listen<boolean>('moonlens://lens-peek', (e) => {
          seen.peek = true;
          peek = e.payload;
        })
      );
      lensControls()
        .then((c) => {
          if (c?.paused && !seen.pause) setPaused(true);
          if (c?.original && !seen.peek) peek = true;
        })
        .catch(() => {});
      unlisteners.push(
        await win.listen<Attachment>(ATTACH_EVENT, (e) => applyAttachment(e.payload))
      );
      windowAttachment(id).then(applyAttachment).catch(() => {});
      // Operate-mode hotkey (all windows at once).
      unlisteners.push(await win.listen<boolean>('moonlens://lens-operate', (e) => setOperate(e.payload)));
      // Outside text window: it asks for the content once it listens,
      // and passes on what the user does there.
      unlisteners.push(await win.listen(TEXT_READY_EVENT, () => textReady++));
      unlisteners.push(
        await win.listen<TextCommand>(TEXT_COMMAND_EVENT, ({ payload: c }) => {
          if (c.kind === 'placement') setPlacement(c.value);
          else if (c.kind === 'explain') explain();
          else showText = false;
        })
      );
      unlisteners.push(
        await win.listen<LensLiveEvent>('moonlens://lens-live', (e) => {
          if (continuous) applyLive(e.payload);
        })
      );
    } catch {
      /* not running inside Tauri */
    }
    const ro = new ResizeObserver(() => reportInsets());
    if (headerEl) ro.observe(headerEl);
    unlisteners.push(() => ro.disconnect());
    try {
      unlisteners.push(
        await win.onMoved(({ payload: p }) => {
          const key = `${p.x},${p.y}`;
          if (moved(key, lastPos)) lastPos = key;
        })
      );
      unlisteners.push(
        await win.onResized(({ payload: p }) => {
          const key = `${p.width}x${p.height}`;
          if (moved(key, lastSize)) lastSize = key;
          dpr = window.devicePixelRatio || 1;
          win
            .isMaximized()
            .then((m) => (maximized = m))
            .catch(() => {});
        })
      );
      const pos = await win.outerPosition();
      const size = await win.innerSize();
      lastPos = `${pos.x},${pos.y}`;
      lastSize = `${size.width}x${size.height}`;
    } catch {
      /* not running inside Tauri */
    }
  });

  onDestroy(() => {
    unlisteners.forEach((u) => u?.());
    stopObserving();
    clearTimeout(fitNoteTimer);
    invoke('set_lens_continuous', { id, on: false }).catch(() => {});
  });

  const activity = $derived(
    busy || reading
      ? '文字認識中…'
      : translating
        ? '翻訳中…'
        : empty
          ? '範囲内に文字なし'
          : !hasText
            ? continuous
              ? '画面変化時に自動翻訳'
              : '再取得で翻訳'
            : null
  );
  /** Fitting is the user's own request just now: shown before the rest. */
  const fitStatus = $derived(
    fit.kind === 'observing'
      ? `字幕領域を観測中… 残余 ${remainingSeconds(fit.elapsed, fit.samples.length)} 秒`
      : fitNote
  );
  const pauseStatus = $derived(
    paused ? (pauseKey ? `一時停止中（${pauseKey} で再開）` : '一時停止中') : null
  );
  const status = $derived(pauseStatus ?? fitStatus ?? lastError ?? activity);
</script>

<main class="lens">
  <header bind:this={headerEl} data-tauri-drag-region class="bar">
    <LensActions
      bind:menuOpen
      {continuous}
      {showOriginal}
      {showText}
      {targetLanguage}
      {readMode}
      onreadmode={changeReadMode}
      {hasText}
      busy={working}
      oncontinuous={() => (continuous = !continuous)}
      {operate}
      {operateKey}
      onoperate={() => setOperate(!operate)}
      attachedTo={attachment.target ? targetName(attachment.target) : ''}
      {attachOpen}
      onattach={() => (attachOpen = !attachOpen)}
      onrefresh={refresh}
      onoriginal={() => (showOriginal = !showOriginal)}
      onlanguage={changeLanguage}
      oncopy={copy}
      ontext={() => (showText = !showText)}
      onexplain={explain}
      fit={fit.kind}
      onfit={toggleFit}
      {maximized}
      onmaximize={toggleMaximize}
      onadd={() => openLensBeside(id).catch(() => {})}
      onclose={close}
    >
      {#snippet lead()}
        <div class="lead pointer-events-none flex items-center gap-2" data-tauri-drag-region>
          <span class="title"
            >{attachment.target ? `吸着中：${targetName(attachment.target)}` : 'ウィンドウ翻訳'}</span
          >
          {#if hasText}
            <PrivacyBadge {viaCloud} {provider} />
          {/if}
        </div>
      {/snippet}
    </LensActions>
    {#if attachOpen}
      <AttachMenu
        label={id}
        {attachment}
        docks={ATTACH_DOCKS}
        onclose={() => (attachOpen = false)}
      />
    {/if}
  </header>

  <!-- The observed range. Transparent; the mouse passes through it. -->
  <section
    class="body"
    class:operate
    class:working={working && !paused}
    class:peek
    aria-label="訳文"
    aria-busy={working && !paused}
  >
    {#if !showOriginal && !peek && !paused}
      {#each boxes as b (b.rect.key)}
        <div
          class="box"
          class:pending={b.pending}
          style="left: {b.rect.left}px; top: {b.rect.top}px; width: {b.rect.width}px; height: {b.rect.height}px;
                 background: {rgb(b.block.colors?.background, 'rgba(255,255,255,0.96)')};
                 color: {rgb(b.block.colors?.foreground, '#111')};
                 clip-path: {maskClip(b.block, dpr) || 'none'};"
        >
          {#if !b.pending}
            <FlowText
              block={b.block}
              text={b.block.translation ?? ''}
              scale={dpr}
              grow={b.grow}
              background={rgb(b.block.colors?.background, 'rgba(255,255,255,0.96)')}
            />
          {/if}
        </div>
      {/each}
    {/if}

    {#if textInside}
      <aside class="panel" aria-label="テキスト">
        <LensText
          view={textView}
          onplacement={setPlacement}
          onexplain={explain}
          onclose={() => (showText = false)} />
      </aside>
    {/if}

    {#if status}
      <div class="status" class:error={!pauseStatus && !fitStatus && !!lastError}>{status}</div>
    {/if}
    {#key snapped}
      {#if snapped}<div class="snap" aria-hidden="true"></div>{/if}
    {/key}
    <!-- Resize grip (also clickable while the body lets the mouse through). -->
    {#if !maximized}
      <div class="grip" role="presentation" title="サイズ変更" onpointerdown={startResize}></div>
    {/if}
  </section>
</main>

<style>
  :global(html),
  :global(body) {
    background: transparent;
    margin: 0;
  }
  .lens {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    color: #f4f4f5;
  }
  /* Fixed height: the backend puts this header right above the range
     (lens::HEADER_CSS). */
  .bar {
    /* 縦メニュー（LensActions）の配置基準 */
    position: relative;
    box-sizing: border-box;
    height: 32px;
    flex: none;
    display: flex;
    align-items: center;
    /* 項目間隔：LensActions の MORE（縦メニューボタンの占有幅）と同値 */
    gap: 4px;
    padding: 0 4px;
    background: rgba(24, 24, 28, 0.92);
    border-radius: 8px 8px 0 0;
    user-select: none;
    --la-hover: rgba(255, 255, 255, 0.12);
    --la-pressed: color-mix(in srgb, var(--ml-accent) 45%, transparent);
    --la-bg: rgb(24, 24, 28);
    container-type: inline-size;
  }
  .lead {
    flex: none;
    margin: 0 4px;
  }
  /* 狭い範囲：見出しを省略し操作列へ幅を譲渡 */
  @container (max-width: 300px) {
    .lead {
      display: none;
    }
  }
  /* 既定幅（460px）：見出しの文字を省略し、操作列の全項目を表示 */
  @container (max-width: 520px) {
    .title {
      display: none;
    }
  }
  .title {
    display: block;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 11px;
    opacity: 0.75;
    white-space: nowrap;
  }
  /* 吸着の成立時：枠を 1 回点滅（段階的、不透明度のみの演出） */
  .snap {
    position: absolute;
    inset: 0;
    pointer-events: none;
    box-shadow: inset 0 0 0 4px var(--ml-accent, rgb(139, 92, 246));
    opacity: 0;
    animation: snap 360ms steps(3, jump-none);
  }
  @keyframes snap {
    from {
      opacity: 1;
    }
    to {
      opacity: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .snap {
      animation: none;
    }
  }
  .body {
    position: relative;
    flex: 1;
    overflow: hidden;
    /* Marks the range without covering it. */
    box-shadow: inset 0 0 0 2px color-mix(in srgb, var(--ml-accent) 60%, transparent);
  }
  /* 操作モード：枠を破線へ変更し（配色に依存しない識別）、訳文を選択可能化 */
  .body.operate {
    box-shadow: none;
    outline: 2px dashed color-mix(in srgb, var(--ml-accent) 70%, white);
    outline-offset: -2px;
  }
  .box {
    position: absolute;
    border-radius: 3px;
  }
  .body.operate .box {
    user-select: text;
    -webkit-user-select: text;
    cursor: text;
  }
  /* 処理中（文字認識・翻訳）：斜光が左から右へ反復走査。transform のみの演出（再描画なし） */
  .body.working::before {
    content: '';
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: 45%;
    pointer-events: none;
    background: linear-gradient(
      90deg,
      transparent,
      color-mix(in srgb, var(--ml-accent) 26%, transparent) 40%,
      rgba(255, 255, 255, 0.34) 50%,
      color-mix(in srgb, var(--ml-accent) 26%, transparent) 60%,
      transparent
    );
    transform: translateX(-140%) skewX(-16deg);
    animation: sweep 1.2s linear infinite;
    will-change: transform;
  }
  @keyframes sweep {
    to {
      transform: translateX(330%) skewX(-16deg);
    }
  }
  .box.pending {
    background-image: linear-gradient(100deg, transparent 30%, color-mix(in srgb, var(--ml-accent) 18%, transparent) 50%, transparent 70%) !important;
  }
  /* 動作軽減の設定時：走査を停止し、淡色の単色で表示 */
  @media (prefers-reduced-motion: reduce) {
    .body.working::before {
      animation: none;
      transform: none;
      width: 100%;
      background: color-mix(in srgb, var(--ml-accent) 12%, transparent);
    }
  }
  .panel {
    position: absolute;
    inset: 8px 8px 8px auto;
    width: min(360px, calc(100% - 16px));
    display: flex;
    flex-direction: column;
    border-radius: 10px;
    background: rgba(24, 24, 28, 0.94);
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.4);
  }
  /* 原文表示キーの押下中：範囲上の表示を全て非表示（原文を遮蔽しない） */
  .body.peek .panel,
  .body.peek .status {
    visibility: hidden;
  }
  .status {
    position: absolute;
    left: 8px;
    bottom: 8px;
    max-width: calc(100% - 40px);
    border-radius: 6px;
    padding: 4px 8px;
    font-size: 11px;
    background: rgba(24, 24, 28, 0.85);
  }
  .status.error {
    color: #fca5a5;
  }
  .grip {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 18px;
    height: 18px;
    cursor: nwse-resize;
    background: linear-gradient(135deg, transparent 50%, color-mix(in srgb, var(--ml-accent) 85%, transparent) 50%);
    border-bottom-right-radius: 2px;
  }
</style>
