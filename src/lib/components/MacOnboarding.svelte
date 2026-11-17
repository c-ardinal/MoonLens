<script lang="ts">
  import { onMount } from 'svelte';
  import {
    CheckCircle2,
    CircleAlert,
    CircleHelp,
    Monitor,
    Mic,
    MousePointer,
    Keyboard,
    RefreshCw
  } from 'lucide-svelte';
  import {
    getOnboardingStatus,
    macosPermissionsSnapshot,
    macosRequestScreenCapture,
    markMacosPermissionsReviewed,
    openMacosPrivacyPane,
    type PermissionReport,
    type PermissionStatus
  } from '$lib/ipc';

  type PermissionRow = {
    id: 'screen_capture' | 'accessibility' | 'microphone' | 'input_monitoring';
    title: string;
    description: string;
    icon: any;
    detectable: boolean;
  };

  const rows: PermissionRow[] = [
    {
      id: 'screen_capture',
      title: '画面収録',
      description: 'OCR・ライブ字幕の画面取得に必須。',
      icon: Monitor,
      detectable: true
    },
    {
      id: 'accessibility',
      title: 'アクセシビリティ',
      description: '他アプリ使用中のショートカット起動に必須。',
      icon: MousePointer,
      detectable: true
    },
    {
      id: 'microphone',
      title: 'マイク',
      description: 'マイク入力の文字起こしに必須。',
      icon: Mic,
      detectable: false
    },
    {
      id: 'input_monitoring',
      title: '入力モニタリング',
      description: 'ショートカットの安定動作に必須。',
      icon: Keyboard,
      detectable: false
    }
  ];

  let show = $state(false);
  let perms = $state<PermissionReport>({
    screen_capture: 'unknown',
    accessibility: 'unknown',
    microphone: 'unknown',
    input_monitoring: 'unknown'
  });

  onMount(async () => {
    const status = await getOnboardingStatus();
    show = status.needs_macos_permissions_review;
    perms = status.macos_permissions;
  });

  function statusOf(id: PermissionRow['id']): PermissionStatus {
    return perms[id];
  }

  async function refreshNow() {
    perms = await macosPermissionsSnapshot();
    // If the user has now granted every detectable permission, close
    // the modal automatically — there is nothing left to walk through.
    if (perms.screen_capture === 'granted' && perms.accessibility === 'granted') {
      await markMacosPermissionsReviewed();
      show = false;
    }
  }

  async function openPane(id: PermissionRow['id']) {
    await openMacosPrivacyPane(id);
  }

  async function requestScreenCapture() {
    const next = await macosRequestScreenCapture();
    perms = { ...perms, screen_capture: next };
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
      <header class="flex items-start justify-between gap-3">
        <div>
          <h2 class="text-base font-semibold">macOS の権限設定</h2>
          <p class="mt-1 text-sm text-ink-muted">
            主要機能に OS 権限が必要。各項目からシステム設定を開き、MoonLens を許可。
          </p>
        </div>
        <button
          class="inline-flex items-center gap-1.5 rounded-ml-sm border border-border bg-surface px-2 py-1 text-xs text-ink-muted hover:border-accent/60 hover:text-ink"
          onclick={refreshNow}
          title="権限状態の再確認"
        >
          <RefreshCw class="h-3.5 w-3.5" />
          再確認
        </button>
      </header>

      <ul class="mt-4 flex flex-col gap-3">
        {#each rows as row (row.id)}
          {@const st = statusOf(row.id)}
          <li
            class="flex items-center justify-between gap-3 rounded-ml border border-border bg-surface p-3"
          >
            <div class="flex items-start gap-3">
              <row.icon class="mt-0.5 h-5 w-5 text-ink-muted" />
              <div>
                <p class="flex items-center gap-2 text-sm font-medium">
                  {row.title}
                  {#if st === 'granted'}
                    <span
                      class="inline-flex items-center gap-1 rounded-full bg-accent-soft px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide text-accent"
                    >
                      <CheckCircle2 class="h-3 w-3" />
                      許可済
                    </span>
                  {:else if st === 'denied'}
                    <span
                      class="inline-flex items-center gap-1 rounded-full bg-red-500/10 px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide text-red-500"
                    >
                      <CircleAlert class="h-3 w-3" />
                      未許可
                    </span>
                  {:else}
                    <span
                      class="inline-flex items-center gap-1 rounded-full bg-surface-muted px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide text-ink-faint"
                    >
                      <CircleHelp class="h-3 w-3" />
                      不明
                    </span>
                  {/if}
                </p>
                <p class="text-xs text-ink-muted">{row.description}</p>
              </div>
            </div>
            <div class="flex flex-col items-stretch gap-1">
              {#if row.id === 'screen_capture' && st !== 'granted'}
                <button
                  class="rounded-ml-sm bg-accent px-3 py-1 text-xs font-medium text-white hover:opacity-90"
                  onclick={requestScreenCapture}
                >
                  許可要求
                </button>
              {/if}
              <button
                class="rounded-ml-sm border border-border bg-surface-elevated px-3 py-1 text-xs text-ink-muted hover:border-accent/60 hover:text-ink"
                onclick={() => openPane(row.id)}
              >
                設定を開く
              </button>
            </div>
          </li>
        {/each}
      </ul>

      <footer class="mt-6 flex items-center justify-end gap-2">
        <button
          class="rounded-ml-sm bg-accent px-3 py-1.5 text-sm font-medium text-white"
          onclick={dismiss}
        >
          完了／後で設定
        </button>
      </footer>
    </div>
  </div>
{/if}
