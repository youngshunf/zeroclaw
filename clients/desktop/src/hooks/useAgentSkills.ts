/**
 * useAgentSkills — 从 Agent 工作区加载技能列表
 *
 * 策略：
 * 1. 通过 agent-api 的 listFiles 获取工作区文件列表
 * 2. 筛选出 skills/xxx/SKILL.md 或 skills/xxx/SKILL.toml 文件
 * 3. 对于 /skill 命令菜单：提供技能 ID 和描述
 *
 * 如果 agent-api 不可用，降级到空列表。
 */
import { useState, useEffect, useCallback } from 'react';
import { listFiles, readFile, listAgents } from '@/lib/agent-api';
import type { SlashCommandItem } from '@/components/chat/input/HxSlashMenu';
import type { MentionItem } from '@/components/chat/input/HxMentionMenu';

export interface SkillInfo {
  /** 技能 ID（目录名） */
  id: string;
  /** 技能名称 */
  name: string;
  /** 技能描述 */
  description: string;
}

/**
 * 从工作区文件列表中提取技能信息
 */
function extractSkillsFromFiles(files: string[]): SkillInfo[] {
  const skills: SkillInfo[] = [];
  const skillDirs = new Set<string>();

  for (const file of files) {
    // 匹配 skills/skill-name/SKILL.md 或 SKILL.toml
    const match = file.match(/^skills\/([^/]+)\/(SKILL\.md|SKILL\.toml)$/);
    if (match && !skillDirs.has(match[1])) {
      skillDirs.add(match[1]);
      skills.push({
        id: match[1],
        name: match[1],
        description: '', // 后续异步填充
      });
    }
  }

  return skills;
}

/**
 * 解析 SKILL.md 的 frontmatter 获取 name 和 description
 */
function parseSkillFrontmatter(content: string): { name?: string; description?: string } {
  const lines = content.split('\n');
  if (lines[0]?.trim() !== '---') return {};

  let name: string | undefined;
  let description: string | undefined;

  for (let i = 1; i < lines.length; i++) {
    const line = lines[i];
    if (line.trim() === '---') break;

    const [key, ...rest] = line.split(':');
    const val = rest.join(':').trim().replace(/^["']|["']$/g, '');

    if (key?.trim() === 'name') name = val;
    if (key?.trim() === 'description') description = val;
  }

  return { name, description };
}

export interface UseAgentSkillsReturn {
  skills: SkillInfo[];
  loading: boolean;
  /** 转换为 SlashCommandItem（给 /skill 子菜单用） */
  asSlashItems: SlashCommandItem[];
  /** 转换为 MentionItem（给 @ 菜单用） */
  asMentionItems: MentionItem[];
  refresh: () => void;
}

export function useAgentSkills(): UseAgentSkillsReturn {
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [loading, setLoading] = useState(false);

  const fetchSkills = useCallback(async () => {
    setLoading(true);
    try {
      // 获取当前 agent name
      const agentList = await listAgents();
      const agentName = agentList.current || 'default';

      // 获取工作区文件列表
      const files = await listFiles(agentName);
      const extracted = extractSkillsFromFiles(files);

      // 异步填充 description（从 SKILL.md 的 frontmatter 读取）
      const enriched = await Promise.all(
        extracted.map(async (skill) => {
          try {
            const content = await readFile(agentName, `skills/${skill.id}/SKILL.md`);
            const meta = parseSkillFrontmatter(content);
            return {
              ...skill,
              name: meta.name || skill.id,
              description: meta.description || `技能: ${skill.id}`,
            };
          } catch {
            return { ...skill, description: `技能: ${skill.id}` };
          }
        }),
      );

      setSkills(enriched);
    } catch (err) {
      console.warn('[useAgentSkills] Failed to load skills:', err);
      // 降级到空列表（不阻塞 UI）
      setSkills([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { fetchSkills(); }, [fetchSkills]);

  const asSlashItems: SlashCommandItem[] = skills.map(s => ({
    id: `skill_${s.id}`,
    label: s.name,
    description: s.description,
    hasArgs: true,
    icon: undefined, // 使用默认图标
  }));

  const asMentionItems: MentionItem[] = skills.map(s => ({
    type: 'skill' as const,
    id: s.id,
    label: s.name,
    description: s.description,
  }));

  return { skills, loading, asSlashItems, asMentionItems, refresh: fetchSkills };
}
