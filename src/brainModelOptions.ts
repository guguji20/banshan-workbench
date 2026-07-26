import type { AiCredentialStatus } from "./generated/bsaigc/AiCredentialStatus";
import type { BrainModelOption } from "./components/BrainCenter";

export function buildBrainModelOptions(
  status: AiCredentialStatus | null,
): readonly BrainModelOption[] {
  const provider =
    status?.providers.find(
      (candidate) => candidate.id === status.defaultProviderId,
    ) ??
    status?.providers.find((candidate) => candidate.isDefault) ??
    null;
  const defaultModel =
    status?.defaultModel?.trim() || provider?.defaultModel.trim() || "";
  const options: BrainModelOption[] = [
    {
      id: "default",
      label: defaultModel ? `跟随设置 · ${defaultModel}` : "跟随设置",
    },
  ];
  const seen = new Set(["default"]);

  for (const rawModel of provider?.models ?? []) {
    const model = rawModel.trim();
    if (!model || seen.has(model)) continue;
    seen.add(model);
    options.push({ id: model, label: model });
  }

  return options;
}

export function normalizeBrainModelSelection(
  selectedModel: string,
  models: readonly BrainModelOption[],
): string {
  return models.some(
    (model) => model.id === selectedModel && model.available !== false,
  )
    ? selectedModel
    : "default";
}
