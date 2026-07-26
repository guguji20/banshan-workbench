import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import {
  SettingsCenter,
  formatSettingsBytes,
  type SettingsCenterProps,
} from "./SettingsCenter";

function createProps(
  overrides: Partial<SettingsCenterProps> = {},
): SettingsCenterProps {
  return {
    open: true,
    providers: [
      {
        id: "provider-banshan",
        name: "半山 AIGC",
        providerKind: "openAiCompatible",
        baseUrl: "https://bsaigc.example/v1",
        models: ["gpt-4.1", "gpt-4.1-mini"],
        defaultModel: "gpt-4.1",
        isDefault: true,
        apiKeyConfigured: true,
        apiKeyHint: "••••1b18",
        connectionState: "ready",
        connectionMessage: "连接正常",
        lastTestedAt: "2026-07-22T08:30:00.000Z",
      },
    ],
    onClose: vi.fn(),
    onCreateProvider: vi.fn(() => undefined),
    onUpdateProvider: vi.fn(() => undefined),
    onDeleteProvider: vi.fn(() => undefined),
    onSetDefaultProvider: vi.fn(() => undefined),
    onTestProviderConnection: vi.fn(() => ({
      state: "ready" as const,
      message: "连接正常",
      models: ["gpt-4.1", "gpt-4.1-mini"],
    })),
    feishuChannel: {
      state: "planned",
      cliDetected: false,
      authorized: false,
      agentDiscoverable: false,
      detail: "渠道接口已预留",
    },
    onRefreshFeishuChannel: vi.fn(() => undefined),
    storageLocations: [
      {
        id: "ledger",
        label: "SQLite Ledger",
        path: "bsaigc-storage://ledger",
        sizeBytes: 1_048_576,
        kind: "ledger",
        authoritative: true,
      },
      {
        id: "vault",
        label: "Local Vault",
        path: "bsaigc-storage://vault",
        sizeBytes: 2_147_483_648,
        kind: "vault",
        authoritative: true,
      },
    ],
    cacheTargets: [
      {
        id: "cache",
        label: "预览与缩略图",
        path: "bsaigc-storage://cache",
        sizeBytes: 536_870_912,
        enabled: true,
        selectedByDefault: true,
      },
    ],
    onOpenStorageLocation: vi.fn(() => undefined),
    onClearCache: vi.fn(() => undefined),
    r2Backup: {
      state: "not_configured",
      configured: false,
      pendingItems: 0,
      detail: "配置壳已就绪",
    },
    onOpenR2Settings: vi.fn(() => undefined),
    update: {
      appVersion: "1.0.0",
      buildChannel: "development",
      buildVersion: "1.0.0-dev.1",
      codexVersion: "0.144.5",
      updateSource: null,
      updateSourceConfigured: false,
      checkState: "idle",
    },
    onCheckForUpdates: vi.fn(() => undefined),
    ...overrides,
  };
}

describe("SettingsCenter", () => {
  it("does not render while closed", () => {
    const html = renderToStaticMarkup(
      <SettingsCenter {...createProps({ open: false })} />,
    );

    expect(html).toBe("");
  });

  it("renders the compact five-category shell and provider editor", () => {
    const html = renderToStaticMarkup(<SettingsCenter {...createProps()} />);

    expect(html).toContain("AI 服务");
    expect(html).toContain("渠道");
    expect(html).toContain("存储与缓存");
    expect(html).toContain("云备份");
    expect(html).toContain("更新与关于");
    expect(html).toContain("半山 AIGC");
    expect(html).toContain("https://bsaigc.example/v1");
    expect(html).toContain("gpt-4.1-mini");
    expect(html).toContain("拉取模型");
    expect(html).toContain("可手动添加或从服务拉取");
    expect(html).toContain('tabindex="-1"');
    expect(html).toContain("data-settings-initial-focus");
    expect(html).toContain("移除已保存密钥");
    expect(html).not.toContain("sk-live-secret");
  });

  it("renders authoritative paths and whitelist-only cache cleanup", () => {
    const html = renderToStaticMarkup(
      <SettingsCenter
        {...createProps({ initialCategory: "storage" })}
      />,
    );

    expect(html).toContain("SQLite Ledger");
    expect(html).toContain("Local Vault");
    expect(html).toContain("权威数据");
    expect(html).toContain("预览与缩略图");
    expect(html).toContain("bsaigc-storage://ledger");
    expect(html).toContain("预计释放 512 MB");
    expect(html).toContain("不触碰账本、原件和凭据");
    expect(html).toContain("清理所选缓存");
  });

  it("keeps Feishu and R2 as non-blocking integration shells", () => {
    const channelHtml = renderToStaticMarkup(
      <SettingsCenter
        {...createProps({ initialCategory: "channels" })}
      />,
    );
    const backupHtml = renderToStaticMarkup(
      <SettingsCenter
        {...createProps({ initialCategory: "backup" })}
      />,
    );

    expect(channelHtml).toContain("飞书 CLI");
    expect(channelHtml).toContain("本版本仅保留渠道接口");
    expect(channelHtml).toContain("接口预留");
    expect(channelHtml).not.toContain("配置入口");
    expect(backupHtml).toContain("Cloudflare R2");
    expect(backupHtml).toContain("仅异步备份");
    expect(backupHtml).toContain("本地先落盘");
  });

  it("shows build, runtime and signed-update source state", () => {
    const html = renderToStaticMarkup(
      <SettingsCenter
        {...createProps({ initialCategory: "updates" })}
      />,
    );

    expect(html).toContain("应用版本");
    expect(html).toContain("1.0.0");
    expect(html).toContain("1.0.0-dev.1");
    expect(html).toContain("Development");
    expect(html).toContain("Codex Runtime");
    expect(html).toContain("0.144.5");
    expect(html).toContain("更新源未配置");
    expect(html).toContain("配置签名更新源后才会启用安装流程");
    expect(html).toContain("手动检查");
  });

  it("formats storage sizes for compact rows", () => {
    expect(formatSettingsBytes(0)).toBe("0 B");
    expect(formatSettingsBytes(1024)).toBe("1.0 KB");
    expect(formatSettingsBytes(10 * 1024 * 1024)).toBe("10 MB");
  });

  it("never renders local absolute storage paths", () => {
    const html = renderToStaticMarkup(
      <SettingsCenter
        {...createProps({ initialCategory: "storage" })}
      />,
    );

    expect(html).not.toContain("C:\\\\");
    expect(html).not.toContain("file://");
  });
});
