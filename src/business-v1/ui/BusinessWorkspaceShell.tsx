import { useEffect, useMemo, useState } from "react";
import { PRODUCT_NAME } from "../../brand";
import { ChatWorkspace } from "./ChatWorkspace";
import { ContextDrawer } from "./ContextDrawer";
import { WorkspaceSidebar } from "./WorkspaceSidebar";
import type { BusinessWorkspaceShellProps, ContextTab } from "./types";
import "./business-workspace.css";

export function BusinessWorkspaceShell({
  productName = PRODUCT_NAME,
  projects,
  conversations,
  activeProjectId,
  activeConversationId,
  messages,
  context,
  contextTab,
  user,
  composer,
  modelOptions,
  actions,
  onContextTabChange,
  isLoading = false,
}: BusinessWorkspaceShellProps) {
  const [isSidebarOpen, setSidebarOpen] = useState(true);
  const [isContextOpen, setContextOpen] = useState(true);
  const [isCompact, setIsCompact] = useState(false);
  const [internalContextTab, setInternalContextTab] = useState<ContextTab>(contextTab ?? "issues");

  useEffect(() => {
    const media = window.matchMedia("(max-width: 900px)");
    const syncLayout = (matches: boolean) => {
      setIsCompact(matches);
      if (matches) {
        setSidebarOpen(false);
        setContextOpen(false);
      }
    };
    const handleChange = (event: MediaQueryListEvent) => syncLayout(event.matches);

    syncLayout(media.matches);
    media.addEventListener("change", handleChange);
    return () => media.removeEventListener("change", handleChange);
  }, []);

  useEffect(() => {
    if (contextTab) setInternalContextTab(contextTab);
  }, [contextTab]);

  const activeProject = useMemo(
    () => projects.find((project) => project.id === activeProjectId),
    [activeProjectId, projects],
  );
  const activeConversation = useMemo(
    () => conversations.find((conversation) => conversation.id === activeConversationId),
    [activeConversationId, conversations],
  );

  const changeContextTab = (tab: ContextTab) => {
    setInternalContextTab(tab);
    onContextTabChange?.(tab);
  };

  return (
    <div
      className={`bw-shell ${isSidebarOpen ? "has-sidebar" : ""} ${
        isContextOpen ? "has-context" : ""
      }`}
      data-product-shell="business-v1"
      data-loading={isLoading ? "true" : "false"}
    >
      {isSidebarOpen ? (
        <WorkspaceSidebar
          productName={productName}
          projects={projects}
          conversations={conversations}
          activeProjectId={activeProjectId}
          activeConversationId={activeConversationId}
          user={user}
          actions={actions}
          onClose={() => setSidebarOpen(false)}
        />
      ) : null}

      <ChatWorkspace
        project={activeProject}
        conversation={activeConversation}
        messages={messages}
        composer={composer}
        modelOptions={modelOptions}
        actions={actions}
        isSidebarOpen={isSidebarOpen}
        isContextOpen={isContextOpen}
        onOpenSidebar={() => {
          if (isCompact) setContextOpen(false);
          setSidebarOpen(true);
        }}
        onToggleContext={() => {
          if (isCompact) setSidebarOpen(false);
          setContextOpen((value) => !value);
        }}
      />

      {isContextOpen ? (
        <ContextDrawer
          context={context}
          activeTab={internalContextTab}
          actions={actions}
          onTabChange={changeContextTab}
          onClose={() => setContextOpen(false)}
        />
      ) : null}

      {(isSidebarOpen || isContextOpen) ? (
        <button
          className="bw-mobile-scrim"
          type="button"
          onClick={() => { setSidebarOpen(false); setContextOpen(false); }}
          aria-label="关闭侧栏"
        />
      ) : null}
    </div>
  );
}
