import type { Config } from 'tailwindcss';

export default {
  content: ['./src/**/*.{html,svelte,ts,js}'],
  darkMode: 'class',
  theme: {
    extend: {
      fontFamily: {
        sans: [
          'Geist',
          'Inter Variable',
          'Inter',
          'Noto Sans JP',
          'system-ui',
          'sans-serif'
        ],
        mono: ['Geist Mono', 'JetBrains Mono', 'ui-monospace', 'monospace']
      },
      colors: {
        // Legacy aliases — still used by existing components.
        surface: {
          DEFAULT: 'var(--ml-surface)',
          elevated: 'var(--ml-surface-elevated)',
          muted: 'var(--ml-surface-muted)'
        },
        border: 'var(--ml-border)',
        ink: {
          DEFAULT: 'var(--ml-ink)',
          muted: 'var(--ml-ink-muted)',
          faint: 'var(--ml-ink-faint)'
        },
        accent: {
          DEFAULT: 'var(--ml-accent)',
          soft: 'var(--ml-accent-soft)',
          strong: 'var(--accent-strong)',
          ink: 'var(--accent-ink)'
        },
        // Design v2026.05 4-level scale.
        bg: {
          0: 'var(--bg-0)',
          1: 'var(--bg-1)',
          2: 'var(--bg-2)',
          3: 'var(--bg-3)',
          4: 'var(--bg-4)'
        },
        line: {
          DEFAULT: 'var(--line)',
          strong: 'var(--line-strong)'
        }
      },
      borderRadius: {
        ml: '14px',
        'ml-sm': '6px',
        'ml-md': '10px',
        'ml-lg': '14px',
        'ml-xl': '20px'
      },
      boxShadow: {
        ml: 'var(--shadow-md)',
        'ml-lg': 'var(--shadow-lg)',
        'ml-sm': 'var(--shadow-sm)'
      },
      spacing: {
        'ml-1': '4px',
        'ml-2': '8px',
        'ml-3': '12px',
        'ml-4': '16px',
        'ml-5': '20px',
        'ml-6': '24px',
        'ml-8': '32px'
      },
      transitionTimingFunction: {
        ml: 'var(--ease-out)',
        spring: 'var(--ease-spring)'
      }
    }
  },
  plugins: []
} satisfies Config;
