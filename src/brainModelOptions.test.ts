import { describe, expect, it } from "vitest";
import type { AiCredentialStatus } from "./generated/bsaigc/AiCredentialStatus";
import {
  buildBrainModelOptions,
  normalizeBrainModelSelection,
} from "./brainModelOptions";

function credentialStatus(
  overrides: Partial<AiCredentialStatus> = {},
): AiCredentialStatus {
  return {
    provider: "半山 AIGC",
    configured: true,
    persisted: true,
    protection: "windowsDpapiCurrentUser",
    revision: 1,
    updatedAt: 1,
    appliesOnNextRuntimeStart: false,
    defaultProviderId: "bsaigc",
    defaultModel: "gpt-5.6-sol",
    providers: [
      {
        id: "bsaigc",
        name: "半山 AIGC",
        kind: "openAiCompatible",
        baseUrl: "https://example.invalid/v1",
        apiKeyConfigured: true,
        apiKeyHint: "sk-***",
        models: ["gpt-5.6-sol", "gpt-5.5", "gpt-5.6-sol", " "],
        defaultModel: "gpt-5.6-sol",
        isDefault: true,
        enabled: true,
        connection: {
          state: "untested",
          message: "尚未测试连接",
          latencyMs: null,
          testedAt: null,
          discoveredModels: [],
        },
        createdAt: 1,
        updatedAt: 1,
      },
    ],
    ...overrides,
  };
}

describe("brain model options", () => {
  it("follows the selected provider and removes duplicate model ids", () => {
    expect(buildBrainModelOptions(credentialStatus())).toEqual([
      { id: "default", label: "跟随设置 · gpt-5.6-sol" },
      { id: "gpt-5.6-sol", label: "gpt-5.6-sol" },
      { id: "gpt-5.5", label: "gpt-5.5" },
    ]);
  });

  it("keeps a safe default while provider state is loading", () => {
    expect(buildBrainModelOptions(null)).toEqual([
      { id: "default", label: "跟随设置" },
    ]);
  });

  it("falls back when a provider switch removes the selected model", () => {
    const models = buildBrainModelOptions(credentialStatus());
    expect(normalizeBrainModelSelection("gpt-5.5", models)).toBe("gpt-5.5");
    expect(normalizeBrainModelSelection("removed-model", models)).toBe("default");
  });
});
