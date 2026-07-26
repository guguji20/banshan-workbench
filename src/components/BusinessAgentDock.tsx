import { useState } from "react";
import {
  BadgeDollarSign,
  Bot,
  FileSignature,
  Handshake,
  ReceiptText,
  Sparkles,
} from "lucide-react";
import {
  BUSINESS_AGENTS,
  buildBusinessAgentContext,
  buildBusinessAgentPrompt,
  type BusinessAgentContextInput,
  type BusinessAgentDefinition,
} from "./businessAgents";
import "./BusinessAgentDock.css";

const AGENT_ICONS: Record<string, typeof Bot> = {
  negotiation: Handshake,
  quotation: BadgeDollarSign,
  "contract-review": FileSignature,
  "acceptance-billing": ReceiptText,
};

export interface BusinessAgentDockProps {
  context: BusinessAgentContextInput;
  disabled?: boolean;
  onComposeSkill: (text: string) => void;
}

export function BusinessAgentDock({
  context,
  disabled = false,
  onComposeSkill,
}: BusinessAgentDockProps) {
  const [activeAgentId, setActiveAgentId] = useState<string | null>(null);
  const activeAgent =
    BUSINESS_AGENTS.find((agent) => agent.id === activeAgentId) ?? null;

  const useSkill = (agent: BusinessAgentDefinition, skillId: string) => {
    const skill = agent.skills.find((candidate) => candidate.id === skillId);
    if (!skill) return;
    const contextText = buildBusinessAgentContext(context);
    onComposeSkill(buildBusinessAgentPrompt(agent, skill, contextText));
  };

  return (
    <div className="business-agent-dock" data-active={activeAgentId ?? "none"}>
      <div className="business-agent-dock__row" role="tablist" aria-label="商务智能体">
        <span className="business-agent-dock__label">
          <Sparkles size={13} />
          商务智能体
        </span>
        {BUSINESS_AGENTS.map((agent) => {
          const Icon = AGENT_ICONS[agent.id] ?? Bot;
          const isActive = agent.id === activeAgentId;
          return (
            <button
              key={agent.id}
              type="button"
              role="tab"
              aria-selected={isActive}
              className={isActive ? "is-active" : ""}
              onClick={() => setActiveAgentId(isActive ? null : agent.id)}
              title={agent.tagline}
            >
              <Icon size={14} />
              {agent.name}
            </button>
          );
        })}
      </div>
      {activeAgent && (
        <div className="business-agent-dock__skills" role="tabpanel">
          <small>{activeAgent.tagline}——点一下，帮你把要说的话写进输入框，看一眼没问题就发送：</small>
          <div className="business-agent-dock__skill-row">
            {activeAgent.skills.map((skill) => (
              <button
                key={skill.id}
                type="button"
                onClick={() => useSkill(activeAgent, skill.id)}
                disabled={disabled}
                title={skill.hint}
              >
                {skill.label}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
