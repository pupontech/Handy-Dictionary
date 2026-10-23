import { expect, test } from "@playwright/test";
import {
  filterPreferredDecoderModels,
  getModelDisplayName,
  getVisibleAvailableModels,
} from "../src/lib/utils/modelList";

const models = Array.from({ length: 13 }, (_, index) => `rank-${index + 1}`);

test("model picker starts with the ten highest-ranked available models", () => {
  const result = getVisibleAvailableModels(models, false, false);

  expect(result.visible).toEqual(models.slice(0, 10));
  expect(result.hiddenCount).toBe(3);
});

test("show-all reveals every available model", () => {
  const result = getVisibleAvailableModels(models, true, false);

  expect(result.visible).toEqual(models);
  expect(result.hiddenCount).toBe(0);
});

test("active filters reveal every matching model", () => {
  const result = getVisibleAvailableModels(models, false, true);

  expect(result.visible).toEqual(models);
  expect(result.hiddenCount).toBe(0);
});

test("hides redundant Parakeet CTC variants from all picker views", () => {
  const candidates = [
    { id: "parakeet-ctc-0.6b" },
    { id: "parakeet-ctc-1.1b" },
    { id: "parakeet-ctc-future" },
    { id: "parakeet-tdt-0.6b-v3" },
    { id: "parakeet-tdt_ctc-110m" },
    { id: "whisper-medium" },
  ];

  const pickerModels = filterPreferredDecoderModels(candidates);
  expect(pickerModels.map((model) => model.id)).toEqual([
    "parakeet-tdt-0.6b-v3",
    "parakeet-tdt_ctc-110m",
    "whisper-medium",
  ]);
  expect(getVisibleAvailableModels(pickerModels, true, true).visible).toEqual(
    pickerModels,
  );
});

test("keeps a hidden decoder model visible while active or downloading", () => {
  const candidates = [
    { id: "parakeet-ctc-0.6b" },
    { id: "parakeet-ctc-1.1b" },
    { id: "parakeet-tdt-0.6b-v3" },
  ];

  expect(
    filterPreferredDecoderModels(candidates, ["parakeet-ctc-0.6b"]).map(
      (model) => model.id,
    ),
  ).toEqual(["parakeet-ctc-0.6b", "parakeet-tdt-0.6b-v3"]);
  expect(
    filterPreferredDecoderModels(candidates, ["parakeet-ctc-1.1b"]).map(
      (model) => model.id,
    ),
  ).toEqual(["parakeet-ctc-1.1b", "parakeet-tdt-0.6b-v3"]);
});

test("labels TDT-CTC GGUFs by the decoder Handy actually runs", () => {
  expect(
    getModelDisplayName({
      id: "parakeet-tdt_ctc-110m",
      name: "Parakeet TDT-CTC 110M",
    }),
  ).toBe("Parakeet TDT 110M");
  expect(
    getModelDisplayName({
      id: "parakeet-tdt_ctc-1.1b",
      name: "Parakeet TDT-CTC 1.1B",
    }),
  ).toBe("Parakeet TDT 1.1B");
});
