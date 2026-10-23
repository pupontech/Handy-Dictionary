import { expect, test } from "@playwright/test";
import { getVisibleAvailableModels } from "../src/lib/utils/modelList";

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
