<script lang="ts">
  import { onMount } from 'svelte';
  import { CheckCircle2, Monitor, Mic, MousePointer, Keyboard } from 'lucide-svelte';
  import {
    getOnboardingStatus,
    markMacosPermissionsReviewed,
    openMacosPrivacyPane
  } from '$lib/ipc';

  let show = $state(false);

  type PermissionRow = {
    id: 'screen_capture' | 'accessibility' | 'microphone' | 'input_monitoring';
    title: string;
    description: string;
    icon: any;
  };

  const permissions: PermissionRow[] = [
    {
      id: 'screen_capture',
      title: '画面収録',
      description: 'OCR とライブ字幕のスクリーン取得に必要です。',
      icon: Monitor
    },
    {
      id: 'accessibility',
      title: 'アクセシビリティ',
      description: 'グローバルホットキーで他アプリ前面からキャプチャを開始するために必要です。',
      icon: MousePointer
    },
    {
      id: 'microphone',
      title: 'マイク',
      description:
        '音声ループバックを利用しない通常のマイク入力（例: 自分の声を書き起こす）に必要です。',
      icon: Mic
    },
    {
      id: 'input_monitoring',
      title: '入力モニタリング',
      description: 'ホットキーが他アプリ中でも安定して動作するために必要です。',
      icon: Keyboard
    }
  ];

  onMount(async () => {
    const status = await getOnboardingStatus();
    show = status.needs_macos_permissions_review;
  });

  async function open(pane: PermissionRow['id']) {
    await openMacosPrivacyPane(pane);
  }

  async function dismiss() {
    await markMacosPermissionsReviewed();
    show = false;
  }
</script>

{#if show}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6">
    <div
      class="w-full max-w-xl rounded-ml border border-border bg-surface-elevated p-6 shadow-ml-lg"
      role="dialog"
      aria-modal="true"
    >
      <header>
        <h2 class="text-base font-semibold">macOS の権限を許可してください</h2>
        <p class="mt-1 text-sm text-ink-muted">
          MoonLens の中核機能は OS 権限に依存します。各項目の「開く」から
          システム設定を表示し、MoonLens にチェックを入れてください。終わったら「完了」を押すと次回から表示されません。
        </p>
      </header>

      <ul class="mt-4 flex flex-col gap-3">
        {#each permissions as p (p.id)}
          <li
            class="flex items-center justify-between gap-3 rounded-ml border border-border bg-surface p-3"
          >
            <div class="flex items-start gap-3">
              <p.icon class="mt-0.5 h-5 w-5 text-ink-muted" />
              <div>
                <p class="text-sm font-medium">{p.title}</p>
                <p class="text-xs text-ink-muted">{p.description}</p>
              </div>
            </div>
            <button
              class="rounded-ml-sm border border-border bg-surface-elevated px-3 py-1.5 text-sm text-ink-muted hover:border-accent/60 hover:text-ink"
              onclick={() => open(p.id)}
            >
              開く
            </button>
          </li>
        {/each}
      </ul>

      <footer class="mt-6 flex items-center justify-end gap-2">
        <button
          class="rounded-ml-sm bg-accent px-3 py-1.5 text-sm font-medium text-white"
          onclick={dismiss}
        >
          <span class="inline-flex items-center gap-1.5">
            <CheckCircle2 class="h-4 w-4" /> 完了
          </span>
        </button>
      </footer>
    </div>
  </div>
{/if}
