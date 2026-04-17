<script lang="ts">
  let original = $state('');
  let translated = $state('');
</script>

<section
  class="pointer-events-none fixed inset-x-0 bottom-0 mx-auto max-w-3xl p-4"
  aria-live="polite"
>
  <div class="rounded-ml bg-black/70 px-5 py-3 text-center text-white shadow-ml backdrop-blur-xl">
    {#if original}
      <p class="text-xs opacity-70">{original}</p>
    {/if}
    <p class="mt-1 text-xl font-semibold leading-snug">{translated || '音声待機中…'}</p>
  </div>
</section>
