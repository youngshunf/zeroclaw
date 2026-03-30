import React, { useState, useEffect, useCallback } from "react";
import { saveHuanxingSession, type HuanxingLoginData } from "@/config";
import { autoOnboard, registerHasnIdentity, registerHasnAgent, connectHasn } from "@/onboard";
import { sendVerifyCode, phoneLogin } from "@/lib/huanxing-api";
import { startTokenRefresh } from "@/lib/token-refresh";

// Import refactored components
import { StarfieldCanvas } from "@/components/effects/StarfieldCanvas";
import { SupernovaCanvas } from "@/components/effects/SupernovaCanvas";
import { GlowingStar } from "@/components/effects/GlowingStar";
import { OnboardProgress, type OnboardStep } from "@/components/onboard/OnboardProgress";
import { Input } from "@/components/ui/Input";

interface LoginProps {
  onLoginSuccess: (token: string) => void;
}

export default function Login({ onLoginSuccess }: LoginProps) {
  const [supernovaDone, setSupernovaDone] = useState(false);
  const [contentVisible, setContentVisible] = useState(false);
  const [step, setStep] = useState<"phone" | "code" | "onboard">("phone");
  const [phone, setPhone] = useState("");
  const [code, setCode] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [countdown, setCountdown] = useState(0);
  const [loginNickname, setLoginNickname] = useState("星友");

  // Onboard 进度步骤
  const [onboardSteps, setOnboardSteps] = useState<OnboardStep[]>([
    { id: "login", label: "登录验证", status: "pending" },
    { id: "config", label: "创建 AI 引擎配置", status: "pending" },
    { id: "agent", label: "初始化默认助手", status: "pending" },
    { id: "engine", label: "启动 AI 引擎", status: "pending" },
    { id: "hasn", label: "注册 HASN 身份", status: "pending" },
    { id: "hasn_connect", label: "连接 HASN 网络", status: "pending" },
    { id: "ready", label: "一切就绪", status: "pending" },
  ]);

  // 更新某一步的状态
  const updateStep = useCallback(
    (id: string, status: OnboardStep["status"], error?: string) => {
      setOnboardSteps((prev) =>
        prev.map((s) => (s.id === id ? { ...s, status, error } : s))
      );
    },
    []
  );

  useEffect(() => {
    if (countdown <= 0) return;
    const timer = setTimeout(() => setCountdown((c) => c - 1), 1000);
    return () => clearTimeout(timer);
  }, [countdown]);

  const sendCode = useCallback(async () => {
    if (!/^1\d{10}$/.test(phone)) {
      setError("请输入正确的手机号");
      return;
    }
    setLoading(true);
    setError("");
    try {
      await sendVerifyCode(phone);
      setStep("code");
      setCountdown(60);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "验证码发送失败");
    } finally {
      setLoading(false);
    }
  }, [phone]);

  const handleLogin = useCallback(async () => {
    if (code.length !== 6) {
      setError("请输入6位验证码");
      return;
    }
    setLoading(true);
    setError("");

    try {
      // ── Step 1: 登录验证 ──
      updateStep("login", "running");
      let loginData: HuanxingLoginData;
      loginData = await phoneLogin(phone, code);
      const session = saveHuanxingSession(loginData);
      const nickname = loginData.user.nickname || "星友";
      setLoginNickname(nickname);
      updateStep("login", "done");

      // ── 切换到 onboard 进度页面 ──
      setStep("onboard");
      setLoading(false);

      // ── Step 2: 创建配置 ──
      updateStep("config", "running");
      console.log("[huanxing] 登录成功，开始自动 onboard...");
      const onboardResult = await autoOnboard(session);
      console.log("[huanxing] onboard 结果:", onboardResult);

      if (onboardResult.config_created || onboardResult.configUpdated) {
        updateStep("config", "done");
      } else {
        updateStep("config", "done"); // 可能已存在，也算完成
      }

      // ── Step 3: 初始化助手 ──
      updateStep("agent", "running");
      // agent 创建在 onboard 里一起完成了
      await new Promise((r) => setTimeout(r, 300)); // 短暂延迟让 UI 有动画
      if (onboardResult.agent_created !== false) {
        updateStep("agent", "done");
      } else {
        updateStep("agent", "done"); // fallback: 已存在
      }

      // ── Step 4: 启动引擎 ──
      updateStep("engine", "running");
      if (onboardResult.sidecar_started === false && onboardResult.error) {
        updateStep("engine", "error", onboardResult.error);
        // 引擎启动失败不阻塞，用户可以稍后手动启动
        await new Promise((r) => setTimeout(r, 1500));
      } else {
        await new Promise((r) => setTimeout(r, 500));
        updateStep("engine", "done");
      }

      // ── Step 5: 注册 HASN 身份 ──
      updateStep("hasn", "running");
      let hasnIdentity;
      try {
        hasnIdentity = await registerHasnIdentity(session);
        console.log("[huanxing] HASN 身份:", hasnIdentity);

        // 同时注册桌面端默认 Agent 的 HASN 身份
        try {
          const agentId = await registerHasnAgent(
            session,
            "default",
            session.user.nickname ? `${session.user.nickname}的星灵` : "唤星AI助手",
            "local",
          );
          console.log("[huanxing] 默认 Agent HASN 身份:", agentId);
        } catch (agentErr) {
          console.warn("[huanxing] 默认 Agent HASN 注册失败（非致命）:", agentErr);
        }

        updateStep("hasn", "done");
      } catch (err) {
        console.warn("[huanxing] HASN 注册失败（非致命）:", err);
        updateStep("hasn", "error", err instanceof Error ? err.message : "HASN 注册失败");
        // HASN 注册失败不阻塞登录
        hasnIdentity = null;
      }

      // ── Step 6: 连接 HASN 网络 ──
      if (hasnIdentity) {
        updateStep("hasn_connect", "running");
        try {
          await connectHasn(session, hasnIdentity);
          updateStep("hasn_connect", "done");
        } catch (err) {
          console.warn("[huanxing] HASN 连接失败（非致命）:", err);
          updateStep("hasn_connect", "error", err instanceof Error ? err.message : "HASN 连接失败");
        }
      } else {
        updateStep("hasn_connect", "done"); // 跳过
      }

      // ── Step 7: 就绪 ──
      updateStep("ready", "running");
      await new Promise((r) => setTimeout(r, 400));
      updateStep("ready", "done");

      // 启动 token 自动刷新
      startTokenRefresh();

      // 短暂展示完成状态后跳转
      await new Promise((r) => setTimeout(r, 800));
      onLoginSuccess(loginData.access_token);
    } catch (err: unknown) {
      const errMsg = err instanceof Error ? err.message : "登录失败，请重试";
      // 找到当前 running 的步骤标记失败
      setOnboardSteps((prev) => {
        const running = prev.find((s) => s.status === "running");
        if (running) {
          return prev.map((s) =>
            s.id === running.id ? { ...s, status: "error" as const, error: errMsg } : s
          );
        }
        return prev;
      });
      // 如果还在登录阶段，回到验证码输入
      if (step !== "onboard") {
        setError(errMsg);
        setLoading(false);
      } else {
        // onboard 阶段失败，2秒后回到验证码输入
        await new Promise((r) => setTimeout(r, 2000));
        setStep("code");
        setError(errMsg);
        // 重置 onboard 步骤
        setOnboardSteps((prev) =>
          prev.map((s) => ({ ...s, status: "pending" as const, error: undefined }))
        );
      }
    }
  }, [phone, code, onLoginSuccess, updateStep, step]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      if (step === "phone") sendCode();
      else handleLogin();
    }
  };

  return (
    <div className="relative min-h-screen overflow-hidden bg-surface-base transition-colors duration-500">
      {/* 星空 Canvas */}
      <StarfieldCanvas />

      {/* 超新星爆炸入场 */}
      {!supernovaDone && (
        <SupernovaCanvas onDone={() => {
          setContentVisible(true);   // 登录页开始淡入
          setTimeout(() => setSupernovaDone(true), 2500); // 等粒子消融完毕再移除 canvas
        }} />
      )}

      {/* 透明拖拽栏 */}
      <div
        className="fixed left-0 right-0 top-0 h-8 z-[9999] cursor-move select-none"
        style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
        data-tauri-drag-region
      />

      {/* 主内容：爆炸结束后淡入（1s 过渡 + 粒子在上层逐渐消融） */}
      <div className={`transition-opacity duration-1000 ease-out ${contentVisible ? 'opacity-100' : 'opacity-0'}`}>
        {/* 径向背景光 (这里保留了一点固定的品牌紫混合，可以增强视觉层级) */}
        <div className="pointer-events-none fixed z-0 inset-0 opacity-40 dark:opacity-100 bg-[radial-gradient(ellipse_at_50%_30%,rgba(124,58,237,0.08)_0%,transparent_60%),radial-gradient(ellipse_at_30%_70%,rgba(6,182,212,0.04)_0%,transparent_50%)]" />

        {/* 主内容 */}
        <div className="relative flex min-h-screen z-10 flex-col items-center justify-center px-4 pt-7">
          {/* 发光星芒 */}
          <GlowingStar />

          {/* 品牌名 */}
          <h1 className="mb-2 text-center text-2xl font-bold tracking-wider text-content-base md:text-3xl">
            唤星AI
          </h1>
          <p className="mb-8 text-center text-sm text-content-subtle">
            唤醒星辰的力量，AI与你共生
          </p>

          {/* Onboard 进度页 */}
          {step === "onboard" ? (
            <OnboardProgress steps={onboardSteps} nickname={loginNickname} />
          ) : (
            /* 登录卡片 */
            <div
              className="w-full max-w-sm overflow-hidden rounded-2xl border border-border-subtle/60 p-6 backdrop-blur-xl bg-surface-card shadow-[0_0_80px_-20px_rgba(124,58,237,0.15),inset_0_1px_0_rgba(165,180,252,0.08)] dark:shadow-[0_0_80px_-20px_rgba(124,58,237,0.15),inset_0_1px_0_rgba(165,180,252,0.08)]"
            >
              {/* 顶部渐变线 */}
              <div className="absolute left-0 right-0 top-0 h-[2px] bg-gradient-to-r from-transparent via-brand to-transparent opacity-60" />

              {step === "phone" ? (
                <div className="space-y-4">
                  <p className="text-center text-sm text-content-subtle">
                    输入手机号登录
                  </p>
                  <div className="relative">
                    <span className="absolute left-4 top-1/2 -translate-y-1/2 text-sm text-brand">
                      +86
                    </span>
                    <Input
                      type="tel"
                      value={phone}
                      onChange={(e) =>
                        setPhone(e.target.value.replace(/\D/g, "").slice(0, 11))
                      }
                      onKeyDown={handleKeyDown}
                      placeholder="手机号"
                      className="pl-14 pr-4 py-3 !text-base rounded-xl border-border-subtle bg-surface-hover"
                      maxLength={11}
                      autoFocus
                    />
                  </div>
                  {error && (
                    <p className="text-center text-sm text-rose-500 dark:text-rose-400">{error}</p>
                  )}
                  <button
                    type="button"
                    onClick={sendCode}
                    disabled={loading || phone.length !== 11}
                    className="w-full rounded-xl py-3 text-sm font-semibold text-white transition-all duration-300 disabled:opacity-40 hover:brightness-110 hover:shadow-[0_0_24px_rgba(124,58,237,0.4)] bg-gradient-to-br from-brand via-[#4F46E5] to-[#06B6D4]"
                  >
                    {loading ? "发送中..." : "获取验证码"}
                  </button>
                </div>
              ) : (
                <div className="space-y-4">
                  <p className="text-center text-sm text-content-subtle">
                    验证码已发送到{" "}
                    {phone.replace(/(\d{3})\d{4}(\d{4})/, "$1****$2")}
                  </p>
                  <Input
                    type="text"
                    value={code}
                    onChange={(e) =>
                      setCode(e.target.value.replace(/\D/g, "").slice(0, 6))
                    }
                    onKeyDown={handleKeyDown}
                    placeholder="6位验证码"
                    className="text-center !text-2xl tracking-[0.4em] py-3 rounded-xl border-border-subtle bg-surface-hover shadow-inner"
                    maxLength={6}
                    autoFocus
                  />
                  {error && (
                    <p className="text-center text-sm text-rose-500 dark:text-rose-400">{error}</p>
                  )}
                  <button
                    type="button"
                    onClick={handleLogin}
                    disabled={loading || code.length !== 6}
                    className="w-full rounded-xl py-3 text-sm font-semibold text-white transition-all duration-300 disabled:opacity-40 hover:brightness-110 hover:shadow-[0_0_24px_rgba(124,58,237,0.4)] bg-gradient-to-br from-brand via-[#4F46E5] to-[#06B6D4]"
                  >
                    {loading ? "登录中..." : "登录"}
                  </button>
                  <div className="flex items-center justify-between text-sm">
                    <button
                      type="button"
                      onClick={() => {
                        setStep("phone");
                        setCode("");
                        setError("");
                      }}
                      className="text-content-subtle hover:text-content-base transition-colors"
                    >
                      ← 换号码
                    </button>
                    <button
                      type="button"
                      onClick={sendCode}
                      disabled={countdown > 0}
                      className="text-content-subtle hover:text-content-base transition-colors disabled:opacity-40"
                    >
                      {countdown > 0 ? `${countdown}s 后重发` : "重新发送"}
                    </button>
                  </div>
                </div>
              )}

              {/* 协议 */}
              <p className="mt-6 text-center text-xs text-content-muted">
                登录即同意{" "}
                <span className="text-brand/80 cursor-pointer hover:text-brand transition-colors">
                  用户协议
                </span>{" "}
                和{" "}
                <span className="text-brand/80 cursor-pointer hover:text-brand transition-colors">
                  隐私政策
                </span>
              </p>
            </div>
          )}

          {/* 底部版权 */}
          <p className="mt-8 text-center text-xs text-content-muted">
            © 2026 唤星AI · HASN Protocol
          </p>
          <p className="mt-1 text-center text-[10px] opacity-70 text-content-muted">
            OpenClaw 生态产品
          </p>
        </div>
      </div>{/* 主内容包裹结束 */}
    </div>
  );
}
