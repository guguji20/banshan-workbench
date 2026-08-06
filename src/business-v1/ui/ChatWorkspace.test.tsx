import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { BusinessWorkspaceActions, ChatMessage } from "./types";
import { ChatWorkspace, compactMenuNextIndex } from "./ChatWorkspace";

const actions: BusinessWorkspaceActions = {
  onCreateProject: vi.fn(),
  onSelectProject: vi.fn(),
  onProjectAction: vi.fn(),
  onCreateConversation: vi.fn(),
  onSelectConversation: vi.fn(),
  onConversationAction: vi.fn(),
  onStartTask: vi.fn(),
  onOpenWorkspaceFolder: vi.fn(),
  onOpenArtifact: vi.fn(),
  onRetryTask: vi.fn(),
  onConfirmTask: vi.fn(),
  onComposerChange: vi.fn(),
  onSendMessage: vi.fn(),
  onAddFiles: vi.fn(),
  onAddFolder: vi.fn(),
  onPasteScreenshot: vi.fn(),
  onRemoveAttachment: vi.fn(),
  onSourceScopeChange: vi.fn(),
  onNetworkScopeChange: vi.fn(),
  onModelChange: vi.fn(),
  onResolveMissingMaterial: vi.fn(),
  onResolveConflict: vi.fn(),
  onSelectTemplate: vi.fn(),
  onReviewLegalRisk: vi.fn(),
  onPrepareAcceptanceDocuments: vi.fn(),
  onOpenPreview: vi.fn(),
  onApprovalDecision: vi.fn(),
  onRestoreVersion: vi.fn(),
  onOpenHistory: vi.fn(),
  onOpenSettings: vi.fn(),
  onCheckForUpdates: vi.fn(),
  onSignOut: vi.fn(),
};

function renderMessages(messages: ChatMessage[]): string {
  return renderToStaticMarkup(
    <ChatWorkspace
      project={{ id: "project-1", name: "测试项目", customerName: "客户甲", updatedAt: "今天" }}
      conversation={{ id: "thread-1", projectId: "project-1", title: "联网检索", preview: "", updatedAt: "今天" }}
      messages={messages}
      composer={{
        value: "",
        attachments: [],
        sourceScope: "workspace",
        networkScope: "web-enabled",
        modelId: "model-1",
      }}
      modelOptions={[{ value: "model-1", label: "测试模型" }]}
      actions={actions}
      isSidebarOpen={false}
      isContextOpen={false}
      onOpenSidebar={vi.fn()}
      onToggleContext={vi.fn()}
    />,
  );
}

describe("ChatWorkspace web research sources", () => {
  it("renders assistant sources as safe external links with a non-overwrite notice", () => {
    const html = renderMessages([{
      id: "assistant-1",
      role: "assistant",
      authorName: "半山 Agent",
      createdAt: "07/29 16:00",
      content: "已完成联网检索。",
      sources: [{
        id: "source-1",
        url: "https://www.example.com/public-policy",
        title: "公开政策原文",
        domain: "www.example.com",
        accessedAt: Date.UTC(2026, 6, 28, 16, 0, 0),
        accessedDate: "2026年7月29日",
        verificationLabel: "外部未确认",
      }],
    }]);

    expect(html).toContain("联网来源");
    expect(html).toContain("公开政策原文");
    expect(html).toContain("www.example.com");
    expect(html).toContain("访问于 2026年7月29日");
    expect(html).toContain("外部未确认");
    expect(html).toContain("不会自动覆盖客户、合同、报价或其他正式业务数据");
    expect(html).toContain('href="https://www.example.com/public-policy"');
    expect(html).toContain('target="_blank"');
    expect(html).toContain('rel="noopener noreferrer"');
  });

  it("falls back to projecting assistant text without rendering sources for user messages", () => {
    const html = renderMessages([
      {
        id: "assistant-2",
        role: "assistant",
        authorName: "半山 Agent",
        createdAt: "07/29 16:00",
        content: "来源：https://docs.example.com/research/report.html",
      },
      {
        id: "user-1",
        role: "user",
        authorName: "我",
        createdAt: "07/29 16:01",
        content: "不要把 https://private.example.com/input 当成来源卡片",
      },
    ]);

    expect(html).toContain("report");
    expect((html.match(/class="bw-web-source-card"/g) ?? [])).toHaveLength(1);
    expect(html).not.toContain('href="https://private.example.com/input"');
  });
});

describe("ChatWorkspace composer controls", () => {
  it("uses compact listbox triggers without native selects or a visible paste button", () => {
    const html = renderMessages([]);

    expect(html).not.toContain("<select");
    expect(html).not.toContain("粘贴截图");
    expect((html.match(/aria-haspopup="listbox"/g) ?? [])).toHaveLength(3);
    expect(html).toContain('aria-label="资料范围"');
    expect(html).toContain('aria-label="联网范围"');
    expect(html).toContain('aria-label="模型"');
    expect(html).toContain("仅当前项目");
    expect(html).toContain("允许联网");
  });

  it("wraps keyboard navigation across compact menu options", () => {
    expect(compactMenuNextIndex(0, -1, 3)).toBe(2);
    expect(compactMenuNextIndex(2, 1, 3)).toBe(0);
    expect(compactMenuNextIndex(1, 1, 3)).toBe(2);
    expect(compactMenuNextIndex(0, 1, 0)).toBe(0);
  });
});
