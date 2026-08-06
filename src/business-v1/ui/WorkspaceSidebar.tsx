import { useMemo, useState } from "react";
import {
  Archive,
  ChevronDown,
  ChevronRight,
  History,
  LogOut,
  MoreHorizontal,
  PanelLeftClose,
  Pin,
  Plus,
  RefreshCw,
  Search,
  Settings,
  SquarePen,
  Trash2,
} from "lucide-react";
import { PRODUCT_LOGO_PATH } from "../../brand";
import type {
  BusinessWorkspaceActions,
  ConversationAction,
  ProjectAction,
  WorkspaceConversation,
  WorkspaceProject,
  WorkspaceUser,
} from "./types";

interface WorkspaceSidebarProps {
  productName: string;
  projects: WorkspaceProject[];
  conversations: WorkspaceConversation[];
  activeProjectId: string | null;
  activeConversationId: string | null;
  user: WorkspaceUser;
  actions: BusinessWorkspaceActions;
  onClose: () => void;
}

const actionLabels: Record<ProjectAction | ConversationAction, string> = {
  pin: "置顶",
  rename: "重命名",
  archive: "归档",
  delete: "删除",
};

export function WorkspaceSidebar({
  productName,
  projects,
  conversations,
  activeProjectId,
  activeConversationId,
  user,
  actions,
  onClose,
}: WorkspaceSidebarProps) {
  const [query, setQuery] = useState("");
  const [projectsOpen, setProjectsOpen] = useState(true);
  const [conversationsOpen, setConversationsOpen] = useState(true);
  const [menuKey, setMenuKey] = useState<string | null>(null);

  const normalizedQuery = query.trim().toLocaleLowerCase();
  const filteredProjects = useMemo(
    () =>
      projects.filter((project) =>
        `${project.name} ${project.customerName}`
          .toLocaleLowerCase()
          .includes(normalizedQuery),
      ),
    [normalizedQuery, projects],
  );
  const filteredConversations = useMemo(
    () =>
      conversations.filter(
        (conversation) =>
          (!activeProjectId || conversation.projectId === activeProjectId) &&
          `${conversation.title} ${conversation.preview}`
            .toLocaleLowerCase()
            .includes(normalizedQuery),
      ),
    [activeProjectId, conversations, normalizedQuery],
  );

  const runProjectAction = (projectId: string, action: ProjectAction) => {
    setMenuKey(null);
    actions.onProjectAction(projectId, action);
  };

  const runConversationAction = (conversationId: string, action: ConversationAction) => {
    setMenuKey(null);
    actions.onConversationAction(conversationId, action);
  };

  return (
    <aside className="bw-sidebar" aria-label="项目和对话">
      <header className="bw-sidebar__brand">
        <div className="bw-brand-mark" aria-hidden="true">
          <img src={PRODUCT_LOGO_PATH} alt="" />
        </div>
        <strong>{productName}</strong>
        <button className="bw-icon-button bw-sidebar__close" type="button" onClick={onClose} title="收起侧栏">
          <PanelLeftClose size={17} />
        </button>
      </header>

      <div className="bw-sidebar__actions">
        <button className="bw-primary-button" type="button" onClick={actions.onCreateConversation}>
          <SquarePen size={16} />
          新建对话
        </button>
        <button className="bw-icon-button" type="button" onClick={actions.onOpenHistory} title="历史资料">
          <History size={17} />
        </button>
        <button className="bw-icon-button" type="button" onClick={actions.onCreateProject} title="新建项目">
          <Plus size={17} />
        </button>
      </div>

      <label className="bw-search-field">
        <Search size={15} aria-hidden="true" />
        <input
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="搜索项目或对话"
          aria-label="搜索项目或对话"
        />
      </label>

      <div className="bw-sidebar__scroll">
        <SidebarSectionHeader
          label="项目工作区"
          count={filteredProjects.length}
          isOpen={projectsOpen}
          onToggle={() => setProjectsOpen((value) => !value)}
        />
        {projectsOpen ? (
          <div className="bw-nav-list">
            {filteredProjects.map((project) => {
              const menuId = `project:${project.id}`;
              return (
                <div className="bw-nav-row-wrap" key={project.id}>
                  <button
                    className={`bw-nav-row ${project.id === activeProjectId ? "is-active" : ""}`}
                    type="button"
                    onClick={() => actions.onSelectProject(project.id)}
                  >
                    <span className="bw-project-glyph">{project.name.slice(0, 1)}</span>
                    <span className="bw-nav-row__copy">
                      <strong>{project.name}</strong>
                      <small>{project.customerName}</small>
                    </span>
                    {project.isPinned ? <Pin className="bw-nav-pin" size={12} aria-label="已置顶" /> : null}
                    {project.unreadCount ? <span className="bw-unread">{project.unreadCount}</span> : null}
                  </button>
                  <button
                    className="bw-row-menu-trigger"
                    type="button"
                    onClick={() => setMenuKey(menuKey === menuId ? null : menuId)}
                    title="项目操作"
                  >
                    <MoreHorizontal size={15} />
                  </button>
                  {menuKey === menuId ? (
                    <RowActionMenu onAction={(action) => runProjectAction(project.id, action)} />
                  ) : null}
                </div>
              );
            })}
            {filteredProjects.length === 0 ? <SidebarEmpty label="没有匹配的项目" /> : null}
          </div>
        ) : null}

        <SidebarSectionHeader
          label="当前项目对话"
          count={filteredConversations.length}
          isOpen={conversationsOpen}
          onToggle={() => setConversationsOpen((value) => !value)}
        />
        {conversationsOpen ? (
          <div className="bw-nav-list">
            {filteredConversations.map((conversation) => {
              const menuId = `conversation:${conversation.id}`;
              return (
                <div className="bw-nav-row-wrap" key={conversation.id}>
                  <button
                    className={`bw-nav-row bw-nav-row--conversation ${
                      conversation.id === activeConversationId ? "is-active" : ""
                    }`}
                    type="button"
                    onClick={() => actions.onSelectConversation(conversation.id)}
                  >
                    <span className="bw-nav-row__copy">
                      <strong>{conversation.title}</strong>
                      <small>{conversation.preview}</small>
                    </span>
                    <time>{conversation.updatedAt}</time>
                  </button>
                  <button
                    className="bw-row-menu-trigger"
                    type="button"
                    onClick={() => setMenuKey(menuKey === menuId ? null : menuId)}
                    title="对话操作"
                  >
                    <MoreHorizontal size={15} />
                  </button>
                  {menuKey === menuId ? (
                    <RowActionMenu onAction={(action) => runConversationAction(conversation.id, action)} />
                  ) : null}
                </div>
              );
            })}
            {filteredConversations.length === 0 ? <SidebarEmpty label="暂无对话" /> : null}
          </div>
        ) : null}
      </div>

      <footer className="bw-account">
        <div className="bw-account__avatar">{user.initials}</div>
        <div className="bw-account__copy">
          <strong>{user.name}</strong>
          <small>{user.roleLabel}</small>
        </div>
        <div className="bw-account__actions">
          <button className="bw-icon-button" type="button" onClick={actions.onOpenSettings} title="设置">
            <Settings size={16} />
          </button>
          <button className="bw-icon-button bw-update-button" type="button" onClick={actions.onCheckForUpdates} title="检查更新">
            <RefreshCw size={16} />
            {user.updateAvailable ? <span aria-label="有可用更新" /> : null}
          </button>
          <button className="bw-icon-button" type="button" onClick={actions.onSignOut} title="退出登录">
            <LogOut size={16} />
          </button>
        </div>
      </footer>
    </aside>
  );
}

interface SidebarSectionHeaderProps {
  label: string;
  count: number;
  isOpen: boolean;
  onToggle: () => void;
}

function SidebarSectionHeader({ label, count, isOpen, onToggle }: SidebarSectionHeaderProps) {
  const Icon = isOpen ? ChevronDown : ChevronRight;
  return (
    <button className="bw-sidebar-section-title" type="button" onClick={onToggle}>
      <Icon size={14} />
      <span>{label}</span>
      <small>{count}</small>
    </button>
  );
}

interface RowActionMenuProps {
  onAction: (action: ProjectAction | ConversationAction) => void;
}

function RowActionMenu({ onAction }: RowActionMenuProps) {
  const items: Array<{ action: ProjectAction | ConversationAction; icon: typeof Pin }> = [
    { action: "pin", icon: Pin },
    { action: "rename", icon: SquarePen },
    { action: "archive", icon: Archive },
    { action: "delete", icon: Trash2 },
  ];

  return (
    <div className="bw-row-menu" role="menu">
      {items.map(({ action, icon: Icon }) => (
        <button
          className={action === "delete" ? "is-danger" : ""}
          type="button"
          role="menuitem"
          key={action}
          onClick={() => onAction(action)}
        >
          <Icon size={14} />
          {actionLabels[action]}
        </button>
      ))}
    </div>
  );
}

function SidebarEmpty({ label }: { label: string }) {
  return <div className="bw-sidebar-empty">{label}</div>;
}
