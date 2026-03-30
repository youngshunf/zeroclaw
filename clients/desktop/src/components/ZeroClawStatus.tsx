/**
 * ZeroClaw 连接状态组件
 *
 * 显示 ZeroClaw sidecar 的运行状态和连接信息。
 * 点击跳转到引擎管理页面。
 */
import { useState, useEffect, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { Wifi, WifiOff, RefreshCw } from "lucide-react";
import { apiFetch } from "@/lib/api";

interface ZeroClawHealth {
  status: string;
  version?: string;
  uptime_seconds?: number;
  model?: string;
  provider?: string;
}

export default function ZeroClawStatus() {
  const [health, setHealth] = useState<ZeroClawHealth | null>(null);
  const [connected, setConnected] = useState(false);
  const [checking, setChecking] = useState(false);
  const navigate = useNavigate();

  const checkHealth = useCallback(async () => {
    setChecking(true);
    try {
      const data = await apiFetch<ZeroClawHealth>("/api/status");
      setHealth(data);
      setConnected(true);
    } catch {
      setHealth(null);
      setConnected(false);
    } finally {
      setChecking(false);
    }
  }, []);

  // 初始检查 + 每 30 秒心跳
  useEffect(() => {
    checkHealth();
    const interval = setInterval(checkHealth, 30_000);
    return () => clearInterval(interval);
  }, [checkHealth]);

  const formatUptime = (seconds?: number): string => {
    if (!seconds) return "";
    if (seconds < 60) return `${seconds}s`;
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
    return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;
  };

  return (
    <div
      className="flex items-center gap-2 text-xs cursor-pointer"
      onClick={() => navigate('/engine')}
      title="引擎管理"
    >
      <div className="flex items-center gap-1.5">
        {connected ? (
          <div className="relative">
            <Wifi className="h-3.5 w-3.5 text-emerald-400" />
            <div className="absolute -top-0.5 -right-0.5 w-1.5 h-1.5 bg-emerald-400 rounded-full" />
          </div>
        ) : (
          <WifiOff className="h-3.5 w-3.5 text-red-400" />
        )}
        <span className={connected ? "text-emerald-400" : "text-red-400"}>
          {connected ? "已连接" : "未连接"}
        </span>
      </div>

      {connected && health && (
        <span className="text-[#5f84cc] truncate">
          {health.model && `${health.model}`}
          {health.uptime_seconds ? ` · ${formatUptime(health.uptime_seconds)}` : ""}
        </span>
      )}

      <button
        onClick={(e) => { e.stopPropagation(); checkHealth(); }}
        disabled={checking}
        className="ml-auto p-1 rounded text-[#5f84cc] hover:text-white transition-colors"
        title="刷新状态"
      >
        <RefreshCw className={`h-3 w-3 ${checking ? "animate-spin" : ""}`} />
      </button>
    </div>
  );
}
