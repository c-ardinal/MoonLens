import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
import ConfigIssues from './ConfigIssues.svelte';

const issues = [
  {
    id: 'providers.ocr.chain[0]',
    path: 'providers.ocr.chain[0]',
    problem: '未登録のプロバイダー「foo」（候補：ocrs、ppocr）',
    fix_label: '既定値に復元'
  },
  { id: 'overlay.opacity', path: 'overlay.opacity', problem: '範囲外 1.5（許容：0.5〜1）', fix_label: '1 に補正' },
  { id: 'x', path: 'x', problem: '読込不可の値', fix_label: null }
];

describe('<ConfigIssues>', () => {
  it('lists each issue with its proposed fix', async () => {
    const { getByText, queryAllByRole } = render(ConfigIssues, { props: { initial: issues } });
    await tick();
    expect(getByText('設定の不正値 3 件')).toBeInTheDocument();
    expect(getByText('未登録のプロバイダー「foo」（候補：ocrs、ppocr）')).toBeInTheDocument();
    expect(getByText('1 に補正')).toBeInTheDocument();
    // Two fixable issues: one button each plus "全件修正".
    expect(queryAllByRole('button').map((b) => b.textContent?.trim())).toEqual([
      '全件修正',
      '既定値に復元',
      '1 に補正'
    ]);
  });

  it('renders nothing without issues', async () => {
    const { container } = render(ConfigIssues);
    await tick();
    expect(container.querySelector('section')).toBeNull();
  });
});
