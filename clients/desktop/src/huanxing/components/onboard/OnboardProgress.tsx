import React from "react";

export interface OnboardStep {
  id: string;
  label: string;
  status: "pending" | "running" | "done" | "error";
  error?: string;
}

interface OnboardProgressProps {
  steps: OnboardStep[];
  nickname: string;
}

export function OnboardProgress({ steps, nickname }: OnboardProgressProps) {
  return (
    <div className="w-full max-w-sm space-y-6">
      <div className="text-center">
        <h2 className="text-lg font-semibold text-content-base">
          欢迎回来，{nickname}
        </h2>
        <p className="mt-1 text-sm text-content-subtle">正在初始化你的 AI 引擎…</p>
      </div>

      <div
        className="overflow-hidden rounded-2xl border border-border-subtle/60 p-5 backdrop-blur-xl bg-surface-card shadow-[0_0_80px_-20px_rgba(124,58,237,0.15),inset_0_1px_0_rgba(165,180,252,0.08)] dark:shadow-[0_0_80px_-20px_rgba(124,58,237,0.15),inset_0_1px_0_rgba(165,180,252,0.08)]"
      >
        <div className="space-y-3">
          {steps.map((s, i) => (
            <div key={s.id} className="flex items-center gap-3">
              {/* 状态图标 */}
              <div className="flex h-7 w-7 flex-shrink-0 items-center justify-center">
                {s.status === "done" && (
                  <svg
                    className="h-5 w-5 text-emerald-500 dark:text-emerald-400"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    strokeWidth={2.5}
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      d="M5 13l4 4L19 7"
                    />
                  </svg>
                )}
                {s.status === "running" && (
                  <div className="h-5 w-5 animate-spin rounded-full border-2 border-brand border-t-transparent" />
                )}
                {s.status === "pending" && (
                  <div className="h-2.5 w-2.5 rounded-full bg-border-subtle" />
                )}
                {s.status === "error" && (
                  <svg
                    className="h-5 w-5 text-rose-500 dark:text-rose-400"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    strokeWidth={2.5}
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      d="M6 18L18 6M6 6l12 12"
                    />
                  </svg>
                )}
              </div>

              {/* 步骤文字 */}
              <div className="flex-1 min-w-0">
                <p
                  className={`text-sm transition-colors ${
                    s.status === "done"
                      ? "text-emerald-500 dark:text-emerald-400"
                      : s.status === "running"
                      ? "text-content-base font-medium"
                      : s.status === "error"
                      ? "text-rose-500 dark:text-rose-400"
                      : "text-content-muted"
                  }`}
                >
                  {s.label}
                </p>
                {s.error && (
                  <p className="mt-0.5 text-xs text-rose-500/70 dark:text-rose-400/70 truncate">
                    {s.error}
                  </p>
                )}
              </div>

              {/* 序号 */}
              <span className="flex-shrink-0 text-xs text-content-muted/80">
                {i + 1}/{steps.length}
              </span>
            </div>
          ))}
        </div>

        {/* 底部进度条 */}
        <div className="mt-4 h-1 w-full overflow-hidden rounded-full bg-surface-hover border border-border-subtle/30">
          <div
            className="h-full rounded-full transition-all duration-500 ease-out bg-gradient-to-r from-brand via-[#4F46E5] to-[#06B6D4]"
            style={{
              width: `${
                (steps.filter((s) => s.status === "done").length /
                  steps.length) *
                100
              }%`,
            }}
          />
        </div>
      </div>
    </div>
  );
}
