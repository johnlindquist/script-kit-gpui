/** SDK scripts that touch real system input and must never enter unattended proof. */
export const SDK_SYSTEM_INPUT_TESTS = [
  "test-system.ts",
  "test-clipboard-image.ts",
  "test-scroll-perf.ts",
] as const;
