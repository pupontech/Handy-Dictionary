export const DEFAULT_VISIBLE_AVAILABLE_MODELS = 10;

export interface VisibleAvailableModels<T> {
  visible: T[];
  hiddenCount: number;
}

/**
 * ModelInfo arrays arrive in editorial rank order. Keep the top-ranked models
 * visible until the user asks for the full catalog; filters should expose all
 * matching results so a lower-ranked match is never hidden.
 */
export function getVisibleAvailableModels<T>(
  models: readonly T[],
  showAll: boolean,
  filtersActive: boolean,
): VisibleAvailableModels<T> {
  const visible =
    showAll || filtersActive
      ? [...models]
      : models.slice(0, DEFAULT_VISIBLE_AVAILABLE_MODELS);

  return {
    visible,
    hiddenCount: models.length - visible.length,
  };
}
