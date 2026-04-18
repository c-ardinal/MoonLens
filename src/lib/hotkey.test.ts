import { describe, expect, it } from 'vitest';
import { formatAccelerator, isValidAccelerator } from './hotkey';

describe('formatAccelerator', () => {
  it('turns Tauri accelerator into macOS-like symbols', () => {
    expect(formatAccelerator('CommandOrControl+Shift+Space')).toBe('⌘⇧Space');
  });

  it('passes through unrecognized key names verbatim', () => {
    expect(formatAccelerator('Control+K')).toBe('⌃K');
    expect(formatAccelerator('Shift+F1')).toBe('⇧F1');
  });

  it('renders empty input as empty string', () => {
    expect(formatAccelerator('')).toBe('');
  });

  it('handles single-token accelerators', () => {
    expect(formatAccelerator('F5')).toBe('F5');
  });

  it('renders arrow keys using unicode arrows', () => {
    expect(formatAccelerator('Control+ArrowLeft')).toBe('⌃←');
  });
});

describe('isValidAccelerator', () => {
  it('requires at least one modifier and exactly one key', () => {
    expect(isValidAccelerator('Shift+A')).toBe(true);
    expect(isValidAccelerator('CommandOrControl+Shift+Space')).toBe(true);
  });

  it('rejects empty strings', () => {
    expect(isValidAccelerator('')).toBe(false);
  });

  it('rejects key-only accelerators', () => {
    expect(isValidAccelerator('Space')).toBe(false);
  });

  it('rejects accelerators with no key', () => {
    expect(isValidAccelerator('Shift+Ctrl')).toBe(false);
  });

  it('rejects accelerators with two keys', () => {
    expect(isValidAccelerator('Shift+A+B')).toBe(false);
  });

  it('rejects accelerators with empty segments', () => {
    expect(isValidAccelerator('Shift++A')).toBe(false);
    expect(isValidAccelerator('+Shift+A')).toBe(false);
  });
});
