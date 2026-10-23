export const DEFAULT_VISIBLE_AVAILABLE_MODELS = 10;

// Prefer TDT for Parakeet in this app. The standalone CTC checkpoints duplicate
// the same English/large-model use cases, while the bundled TDT-CTC GGUFs run
// only their TDT decoder. Keep a legacy CTC model visible only while it is
// currently selected or a download is already in progress.
const isStandaloneParakeetCtcModel = (modelId: string) =>
  modelId.startsWith("parakeet-ctc-");

const MODEL_DISPLAY_NAME_OVERRIDES: Readonly<Record<string, string>> = {
  "parakeet-tdt_ctc-110m": "Parakeet TDT 110M",
  "parakeet-tdt_ctc-1.1b": "Parakeet TDT 1.1B",
};

export function filterPreferredDecoderModels<T extends { id: string }>(
  models: readonly T[],
  preservedModelIds: readonly string[] = [],
): T[] {
  const preserved = new Set(preservedModelIds);
  return models.filter(
    (model) =>
      !isStandaloneParakeetCtcModel(model.id) || preserved.has(model.id),
  );
}

export function getModelDisplayName<T extends { id: string; name: string }>(
  model: T,
): string {
  return MODEL_DISPLAY_NAME_OVERRIDES[model.id] ?? model.name;
}

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
