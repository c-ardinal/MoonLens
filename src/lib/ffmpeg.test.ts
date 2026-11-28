import { describe, expect, it } from 'vitest';
import { ffmpegProblem } from './ffmpeg';

describe('ffmpegProblem', () => {
  it('names why ffmpeg cannot be used, or nothing when it can', () => {
    expect(ffmpegProblem({ path: null, check: null })).toBe('file-burn:no_ffmpeg');
    expect(ffmpegProblem({ path: 'C:/f/ffmpeg.exe', check: 'not_ffmpeg' })).toBe('file-burn:not_ffmpeg');
    expect(ffmpegProblem({ path: 'C:/f/ffmpeg.exe', check: 'no_libass' })).toBe('file-burn:no_libass');
    expect(ffmpegProblem({ path: 'C:/f/ffmpeg.exe', check: 'ok' })).toBeNull();
  });
});
