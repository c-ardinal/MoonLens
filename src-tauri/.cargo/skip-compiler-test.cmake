# CMake toolchain file: skip the compiler-test probe.
#
# Why this file exists:
# llama-cpp-sys-2's `vulkan-shaders-gen` ExternalProject spawns a
# fresh inner cmake to build a host helper tool. That inner cmake
# runs `CMakeTestCCompiler.cmake`, which compiles a trivial .c and
# tries to LINK it. On Windows + MSVC the link goes through
# cmake's `vs_link_exe` wrapper which forces `/MANIFEST
# /MANIFESTFILE:...intermediate.manifest`; for the cmTC_XXX
# scratch project this consistently fails with `LNK1104: cannot
# open intermediate.manifest` (debug builds only — release works
# because of slightly different link flags). The wrapper is
# upstream cmake source and can't be patched per-project.
#
# Setting `CMAKE_C_COMPILER_WORKS=1` / `CMAKE_CXX_COMPILER_WORKS=1`
# tells cmake to short-circuit the compiler probe entirely: it
# trusts that the configured compiler works and skips the test
# .c -> .exe round trip. The actual compile + link of the real
# project still happens (and works — we got to [32/523] before
# the test compile failure). This file is wired in via the
# `CMAKE_TOOLCHAIN_FILE` env var in `.cargo/config.toml`, which
# every cmake invocation reads (including ExternalProject spawns).

set(CMAKE_C_COMPILER_WORKS 1 CACHE INTERNAL "Skip CMakeTestCCompiler.cmake (Phase 27 LNK1104 workaround)")
set(CMAKE_CXX_COMPILER_WORKS 1 CACHE INTERNAL "Skip CMakeTestCXXCompiler.cmake (Phase 27 LNK1104 workaround)")
