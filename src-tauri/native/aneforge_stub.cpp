// whisper.cpp can hand its encoder to an external Apple Neural Engine
// library named by environment variables. MoonLens never loads code that
// way, so the hook is compiled as a stub that always reports "unavailable".
#include "aneforge/whisper-aneforge.h"

extern "C" {
struct whisper_aneforge_context * whisper_aneforge_init(const char *) { return nullptr; }
void whisper_aneforge_encode(struct whisper_aneforge_context *, int64_t, int64_t, const float *, float *) {}
void whisper_aneforge_free(struct whisper_aneforge_context *) {}
}
