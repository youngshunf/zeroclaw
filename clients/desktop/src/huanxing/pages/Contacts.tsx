/**
 * 联系人页面 — 适配唤星 Light 主题
 */
import { useState } from "react";
import { Users, UserPlus } from "lucide-react";

export default function Contacts() {
  const [tab, setTab] = useState<"friends" | "requests">("friends");

  return (
    <div style={{ display: 'flex', flex: 1, height: '100%' }}>
      {/* 左侧面板 */}
      <div className="hx-panel">
        <div className="hx-panel-header">
          <h2 className="hx-panel-title">通讯录</h2>
        </div>
        <div style={{ display: 'flex', gap: 4, padding: '0 12px 12px' }}>
          <button
            onClick={() => setTab("friends")}
            className={`hx-nav-item ${tab === "friends" ? "active" : ""}`}
            style={{ width: 'auto', height: 'auto', padding: '6px 12px', borderRadius: 'var(--hx-radius-sm)', gap: 6, display: 'flex', alignItems: 'center', fontSize: 13, fontWeight: 500 }}
          >
            <Users size={15} />
            好友
          </button>
          <button
            onClick={() => setTab("requests")}
            className={`hx-nav-item ${tab === "requests" ? "active" : ""}`}
            style={{ width: 'auto', height: 'auto', padding: '6px 12px', borderRadius: 'var(--hx-radius-sm)', gap: 6, display: 'flex', alignItems: 'center', fontSize: 13, fontWeight: 500 }}
          >
            <UserPlus size={15} />
            请求
          </button>
        </div>
        <div className="hx-conv-list">
          {/* Placeholder */}
          <div className="hx-empty-state" style={{ padding: '60px 0' }}>
            <Users size={40} style={{ opacity: 0.3 }} />
            <p style={{ fontSize: 13, color: 'var(--hx-text-tertiary)' }}>
              {tab === "friends" ? "暂无好友" : "暂无好友请求"}
            </p>
          </div>
        </div>
      </div>

      {/* 右侧内容 */}
      <div className="hx-chat">
        <div className="hx-empty-state">
          <div className="icon">👥</div>
          <h3>通讯录</h3>
          <p>选择好友开始聊天，或查看好友请求</p>
          <p style={{ fontSize: 12, color: 'var(--hx-text-tertiary)', marginTop: 8 }}>HASN Protocol · Phase 2 开发中</p>
        </div>
      </div>
    </div>
  );
}
