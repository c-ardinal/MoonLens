import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render } from '@testing-library/svelte';
import ContextMenu from './ContextMenu.svelte';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(async () => 'clip') }));

function mount(html: string) {
  const host = document.createElement('div');
  host.innerHTML = html;
  document.body.append(host);
  const view = render(ContextMenu);
  return { host, ...view };
}

const labels = () =>
  [...document.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')].map(
    (b) => `${b.textContent?.trim()}${b.disabled ? '(無効)' : ''}`
  );

describe('<ContextMenu>', () => {
  it('suppresses the browser menu and shows nothing on plain UI', async () => {
    const { host } = mount('<a href="/settings/">設定</a>');
    const allowed = await fireEvent.contextMenu(host.querySelector('a')!);
    expect(allowed).toBe(false);
    expect(labels()).toEqual([]);
  });

  it('offers edit actions in a text field, enabled by its state', async () => {
    const { host } = mount('<textarea>hello</textarea>');
    const area = host.querySelector('textarea')!;
    area.setSelectionRange(0, 0);
    await fireEvent.contextMenu(area);
    expect(labels()).toEqual(['切取(無効)', 'コピー(無効)', '貼付', '全選択']);

    area.setSelectionRange(0, 5);
    await fireEvent.contextMenu(area);
    expect(labels()).toEqual(['切取', 'コピー', '貼付', '全選択']);
  });

  it('keeps a read-only field copy-only', async () => {
    const { host } = mount('<textarea readonly>hello</textarea>');
    const area = host.querySelector('textarea')!;
    area.setSelectionRange(0, 5);
    await fireEvent.contextMenu(area);
    expect(labels()).toEqual(['切取(無効)', 'コピー', '貼付(無効)', '全選択']);
  });

  it('offers copy only for selected display text', async () => {
    const { host } = mount('<p>訳文</p>');
    const p = host.querySelector('p')!;
    const spy = vi.spyOn(window, 'getSelection').mockReturnValue({
      toString: () => '訳文'
    } as Selection);
    await fireEvent.contextMenu(p);
    expect(labels()).toEqual(['コピー']);
    spy.mockRestore();
  });

  it('closes on Escape', async () => {
    const { host } = mount('<textarea>hello</textarea>');
    await fireEvent.contextMenu(host.querySelector('textarea')!);
    expect(labels().length).toBe(4);
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(labels()).toEqual([]);
  });

  it('blocks dragging links, images and display text but not field text', async () => {
    const { host } = mount('<a href="/">ホーム</a><img alt="" /><p>文</p><input type="text" />');
    for (const sel of ['a', 'img', 'p']) {
      expect(await fireEvent.dragStart(host.querySelector(sel)!), sel).toBe(false);
    }
    expect(await fireEvent.dragStart(host.querySelector('p')!.firstChild!)).toBe(false);
    expect(await fireEvent.dragStart(host.querySelector('input')!)).toBe(true);
  });
});
