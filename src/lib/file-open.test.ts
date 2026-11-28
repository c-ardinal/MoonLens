import { describe, expect, it } from 'vitest';
import { fileKind, initialView } from './file-open';
import { IDLE } from './file-subtitle';

describe('fileKind', () => {
  it('routes image extensions to image translation', () => {
    for (const name of ['menu.png', 'C:\\shots\\a.JPG', '/tmp/b.jpeg', 'c.webp']) expect(fileKind(name)).toBe('image');
  });

  it('routes every other file to subtitle generation', () => {
    for (const name of ['movie.mp4', 'talk.MKV', 'song.mp3', 'voice.wav', 'no-extension', 'notes.pdf', 'photo.png.mp4'])
      expect(fileKind(name)).toBe('media');
  });
});

describe('initialView', () => {
  it('shows the drop zone when no job exists', () => {
    expect(initialView(IDLE)).toBe('idle');
  });

  it('shows the job when one is running or finished', () => {
    for (const phase of ['running', 'done', 'cancelled', 'failed'] as const)
      expect(initialView({ ...IDLE, phase })).toBe('media');
  });
});
