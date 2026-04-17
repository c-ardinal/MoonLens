/**
 * Shared spring/easing presets. Keep the set minimal — every motion in the
 * app should resolve to one of these, so animation feels consistent.
 */

export const easings = {
  standard: [0.2, 0.8, 0.2, 1] as const,
  emphasis: [0.16, 1, 0.3, 1] as const
};

export const durations = {
  fast: 120,
  normal: 200,
  slow: 320
} as const;

export const springs = {
  soft: { stiffness: 180, damping: 24, mass: 1 },
  snappy: { stiffness: 320, damping: 28, mass: 1 }
} as const;
