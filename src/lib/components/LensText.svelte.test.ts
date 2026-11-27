import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render } from '@testing-library/svelte';
import LensText from './LensText.svelte';
import type { Explanation, TextView } from '$lib/lens-text';

const view = (explanation: Explanation | null): TextView => ({
  blocks: [],
  translation: '訳文',
  source: 'source',
  explanation,
  placement: 'inside'
});

const props = (explanation: Explanation | null, onexplain = vi.fn()) => ({
  view: view(explanation),
  onplacement: vi.fn(),
  onexplain,
  onclose: vi.fn()
});

describe('<LensText> 内容説明', () => {
  it('shows progress while the explanation is generated', () => {
    const { getByRole, queryByText } = render(LensText, { props: props({ status: 'pending' }) });
    expect(getByRole('status')).toHaveTextContent('説明を生成中');
    expect(queryByText('再試行')).not.toBeInTheDocument();
  });

  it('shows the explanation', () => {
    const { getByText, queryByText } = render(LensText, {
      props: props({ status: 'done', text: '北風と太陽の寓話。' })
    });
    expect(getByText('北風と太陽の寓話。')).toBeInTheDocument();
    expect(queryByText('再試行')).not.toBeInTheDocument();
  });

  it('offers a retry after a failure', async () => {
    const onexplain = vi.fn();
    const { getByRole } = render(LensText, {
      props: props({ status: 'failed', message: '制限時間（180 秒）超過' }, onexplain)
    });
    expect(getByRole('alert')).toHaveTextContent('説明の生成に失敗：制限時間（180 秒）超過');
    await fireEvent.click(getByRole('button', { name: '再試行' }));
    expect(onexplain).toHaveBeenCalledOnce();
  });
});
