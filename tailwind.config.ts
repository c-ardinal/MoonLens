import type { Config } from 'tailwindcss';

export default {
  content: ['./src/**/*.{html,svelte,ts,js}'],
  darkMode: 'class',
  theme: {
    extend: {
      fontFamily: {
        sans: ['Inter Variable', 'Inter', 'system-ui', 'sans-serif'],
        mono: ['JetBrains Mono', 'ui-monospace', 'monospace']
      },
      colors: {
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
          soft: 'var(--ml-accent-soft)'
        }
      },
      borderRadius: {
        ml: '12px',
        'ml-sm': '8px'
      },
      boxShadow: {
        ml: '0 8px 32px rgba(0,0,0,0.12)',
        'ml-lg': '0 16px 48px rgba(0,0,0,0.18)'
      },
      spacing: {
        'ml-1': '8px',
        'ml-2': '12px',
        'ml-3': '16px',
        'ml-4': '24px'
      }
    }
  },
  plugins: []
} satisfies Config;
