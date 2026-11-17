// Thin C entry points over whisper.cpp, so the Rust side never has to
// mirror the layout of `whisper_context_params` / `whisper_full_params`
// (both change between whisper.cpp releases).
#include "whisper.h"

#include <atomic>

extern "C" {

struct whisper_context * mlw_init(const char * path, bool use_gpu, bool flash_attn) {
    whisper_context_params cp = whisper_context_default_params();
    cp.use_gpu    = use_gpu;
    cp.flash_attn = flash_attn;
    return whisper_init_from_file_with_params(path, cp);
}

static bool mlw_abort(void * data) {
    return data != nullptr && reinterpret_cast<std::atomic<bool> *>(data)->load();
}

// Transcribe 16 kHz mono f32 samples. Returns whisper_full's status
// (0 = ok). Segments are read back with whisper_full_get_segment_*.
int mlw_full(
        struct whisper_context * ctx,
        const float * samples,
        int n_samples,
        const char * language,   // nullptr = auto-detect
        int n_threads,           // <= 0 = whisper.cpp default
        int beam_size,           // <= 1 = greedy
        int audio_ctx,           // 0 = full 30 s window
        const char * prompt,     // nullptr = none
        float no_speech_thold,
        void * abort_flag) {     // Rust AtomicBool (same layout as std::atomic<bool>)
    whisper_full_params p = whisper_full_default_params(
        beam_size > 1 ? WHISPER_SAMPLING_BEAM_SEARCH : WHISPER_SAMPLING_GREEDY);
    if (beam_size > 1) {
        p.beam_search.beam_size = beam_size;
    }
    if (n_threads > 0) {
        p.n_threads = n_threads;
    }
    p.language         = language ? language : "auto";
    p.detect_language  = false;
    p.translate        = false;
    // Each chunk is decoded on its own; the caller passes the previous
    // text as `prompt` when it wants continuity.
    p.no_context       = true;
    p.print_progress   = false;
    p.print_realtime   = false;
    p.print_special    = false;
    p.print_timestamps = false;
    p.suppress_blank   = true;
    p.suppress_nst     = true;
    p.audio_ctx        = audio_ctx;
    p.initial_prompt   = prompt;
    p.no_speech_thold  = no_speech_thold;
    p.abort_callback   = mlw_abort;
    p.abort_callback_user_data = abort_flag;
    return whisper_full(ctx, p, samples, n_samples);
}

} // extern "C"
