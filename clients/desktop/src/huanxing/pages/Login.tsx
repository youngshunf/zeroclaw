/**
 * 唤星登录页 — 星空连线 + 发光星芒 + 手机号验证码登录
 *
 * 设计参考唤星官网 Hero 风格：
 * - 深空背景 + 随机星点
 * - 星星之间动态连线（Canvas）
 * - 中央发光八角星芒（品牌 Logo）
 * - 渐变登录卡片
 */
import { useState, useEffect, useCallback, useRef } from "react";
import starSvg from "../assets/huanxing-star.svg";
import { saveHuanxingSession, type HuanxingLoginData } from "../config";
import { autoOnboard } from "../onboard";
import { sendVerifyCode, phoneLogin } from "../lib/huanxing-api";
import { startTokenRefresh } from "../lib/token-refresh";

// ─── 星空背景 Canvas ────────────────────────────────────────────────
interface Star {
  x: number;
  y: number;
  r: number;
  baseAlpha: number;
  alpha: number;
  twinkleSpeed: number;
  twinklePhase: number;
  vx: number;
  vy: number;
}

function StarfieldCanvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const starsRef = useRef<Star[]>([]);
  const animRef = useRef<number>(0);
  const CONNECTION_DIST = 120;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const resize = () => {
      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;
    };
    resize();
    window.addEventListener("resize", resize);

    // 初始化星星
    const count = Math.floor((canvas.width * canvas.height) / 6000);
    const stars: Star[] = [];
    for (let i = 0; i < count; i++) {
      const baseAlpha = 0.15 + Math.random() * 0.6;
      stars.push({
        x: Math.random() * canvas.width,
        y: Math.random() * canvas.height,
        r: 0.4 + Math.random() * 1.6,
        baseAlpha,
        alpha: baseAlpha,
        twinkleSpeed: 0.003 + Math.random() * 0.008,
        twinklePhase: Math.random() * Math.PI * 2,
        vx: (Math.random() - 0.5) * 0.08,
        vy: (Math.random() - 0.5) * 0.08,
      });
    }
    starsRef.current = stars;

    let time = 0;
    const draw = () => {
      time++;
      const w = canvas.width;
      const h = canvas.height;
      ctx.clearRect(0, 0, w, h);

      // 更新 & 绘制星星
      for (const s of stars) {
        s.x += s.vx;
        s.y += s.vy;
        if (s.x < -10) s.x = w + 10;
        if (s.x > w + 10) s.x = -10;
        if (s.y < -10) s.y = h + 10;
        if (s.y > h + 10) s.y = -10;

        // 闪烁
        s.alpha =
          s.baseAlpha *
          (0.5 + 0.5 * Math.sin(time * s.twinkleSpeed + s.twinklePhase));

        ctx.beginPath();
        ctx.arc(s.x, s.y, s.r, 0, Math.PI * 2);
        ctx.fillStyle = `rgba(200,210,255,${s.alpha})`;
        ctx.fill();
      }

      // 绘制连线
      for (let i = 0; i < stars.length; i++) {
        for (let j = i + 1; j < stars.length; j++) {
          const dx = stars[i].x - stars[j].x;
          const dy = stars[i].y - stars[j].y;
          const dist = Math.sqrt(dx * dx + dy * dy);
          if (dist < CONNECTION_DIST) {
            const lineAlpha =
              (1 - dist / CONNECTION_DIST) *
              0.15 *
              Math.min(stars[i].alpha, stars[j].alpha) *
              (0.6 + 0.4 * Math.sin(time * 0.01 + i));
            ctx.beginPath();
            ctx.moveTo(stars[i].x, stars[i].y);
            ctx.lineTo(stars[j].x, stars[j].y);
            ctx.strokeStyle = `rgba(165,180,252,${lineAlpha})`;
            ctx.lineWidth = 0.5;
            ctx.stroke();
          }
        }
      }

      animRef.current = requestAnimationFrame(draw);
    };

    animRef.current = requestAnimationFrame(draw);
    return () => {
      cancelAnimationFrame(animRef.current);
      window.removeEventListener("resize", resize);
    };
  }, []);

  return (
    <canvas
      ref={canvasRef}
      className="pointer-events-none fixed inset-0"
      style={{ zIndex: 0 }}
    />
  );
}

// ─── 发光星芒组件 ────────────────────────────────────────────────────
function GlowingStar() {
  return (
    <div className="relative mx-auto mb-6 flex h-32 w-32 items-center justify-center md:h-40 md:w-40">
      {/* 最外层脉冲光晕 */}
      <div
        className="absolute h-full w-full rounded-full animate-pulse-slow"
        style={{
          background:
            "radial-gradient(circle, rgba(124,58,237,0.25) 0%, rgba(79,70,229,0.1) 40%, transparent 70%)",
        }}
      />
      {/* 中层暖光圈 */}
      <div
        className="absolute h-24 w-24 rounded-full animate-pulse-medium md:h-32 md:w-32"
        style={{
          background:
            "radial-gradient(circle, rgba(255,217,61,0.15) 0%, rgba(124,58,237,0.15) 50%, transparent 70%)",
        }}
      />
      {/* SVG 星芒 Logo */}
      <img
        src={starSvg}
        alt="唤星"
        className="relative h-20 w-20 drop-shadow-[0_0_30px_rgba(124,58,237,0.6)] md:h-24 md:w-24"
        style={{
          filter:
            "drop-shadow(0 0 20px rgba(124,58,237,0.5)) drop-shadow(0 0 60px rgba(79,70,229,0.3))",
        }}
      />
      {/* 中心白点 */}
      <div
        className="absolute h-3 w-3 rounded-full bg-white animate-pulse-fast"
        style={{
          boxShadow:
            "0 0 12px rgba(255,255,255,0.8), 0 0 40px rgba(124,58,237,0.6), 0 0 80px rgba(79,70,229,0.3)",
        }}
      />
    </div>
  );
}

// ─── Onboard 步骤定义 ───────────────────────────────────────────────
interface OnboardStep {
  id: string;
  label: string;
  status: "pending" | "running" | "done" | "error";
  error?: string;
}

// ─── Onboard 进度组件 ───────────────────────────────────────────────
function OnboardProgress({
  steps,
  nickname,
}: {
  steps: OnboardStep[];
  nickname: string;
}) {
  return (
    <div className="w-full max-w-sm space-y-6">
      <div className="text-center">
        <h2 className="text-lg font-semibold text-white">
          欢迎回来，{nickname}
        </h2>
        <p className="mt-1 text-sm text-[#8b9fd8]">正在初始化你的 AI 引擎…</p>
      </div>

      <div
        className="overflow-hidden rounded-2xl border border-[#1e2d5a]/60 p-5 backdrop-blur-xl"
        style={{
          background:
            "linear-gradient(160deg, rgba(10,17,40,0.85), rgba(5,8,22,0.92))",
          boxShadow:
            "0 0 80px -20px rgba(124,58,237,0.15), inset 0 1px 0 rgba(165,180,252,0.08)",
        }}
      >
        <div className="space-y-3">
          {steps.map((s, i) => (
            <div key={s.id} className="flex items-center gap-3">
              {/* 状态图标 */}
              <div className="flex h-7 w-7 flex-shrink-0 items-center justify-center">
                {s.status === "done" && (
                  <svg
                    className="h-5 w-5 text-emerald-400"
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
                  <div className="h-5 w-5 animate-spin rounded-full border-2 border-[#7C3AED] border-t-transparent" />
                )}
                {s.status === "pending" && (
                  <div className="h-2.5 w-2.5 rounded-full bg-[#2a3a5c]" />
                )}
                {s.status === "error" && (
                  <svg
                    className="h-5 w-5 text-rose-400"
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
                      ? "text-emerald-400"
                      : s.status === "running"
                      ? "text-white"
                      : s.status === "error"
                      ? "text-rose-400"
                      : "text-[#4a5f8f]"
                  }`}
                >
                  {s.label}
                </p>
                {s.error && (
                  <p className="mt-0.5 text-xs text-rose-400/70 truncate">
                    {s.error}
                  </p>
                )}
              </div>

              {/* 序号 */}
              <span className="flex-shrink-0 text-xs text-[#3a4a6f]">
                {i + 1}/{steps.length}
              </span>
            </div>
          ))}
        </div>

        {/* 底部进度条 */}
        <div className="mt-4 h-1 w-full overflow-hidden rounded-full bg-[#1a2444]">
          <div
            className="h-full rounded-full transition-all duration-500 ease-out"
            style={{
              width: `${
                (steps.filter((s) => s.status === "done").length /
                  steps.length) *
                100
              }%`,
              background:
                "linear-gradient(90deg, #7C3AED, #4F46E5, #06B6D4)",
            }}
          />
        </div>
      </div>
    </div>
  );
}

// ─── 登录页主体 ──────────────────────────────────────────────────────
interface LoginProps {
  onLoginSuccess: (token: string) => void;
}

export default function Login({ onLoginSuccess }: LoginProps) {
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

      // ── Step 5: 就绪 ──
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
    <div className="relative min-h-screen overflow-hidden bg-[#050816]">
      {/* 星空 Canvas */}
      <StarfieldCanvas />

      {/* 径向背景光 */}
      <div
        className="pointer-events-none fixed inset-0"
        style={{
          background:
            "radial-gradient(ellipse at 50% 30%, rgba(124,58,237,0.08) 0%, transparent 60%), radial-gradient(ellipse at 30% 70%, rgba(6,182,212,0.04) 0%, transparent 50%)",
          zIndex: 1,
        }}
      />

      {/* 主内容 */}
      <div className="relative flex min-h-screen flex-col items-center justify-center px-4" style={{ zIndex: 2 }}>
        {/* 发光星芒 */}
        <GlowingStar />

        {/* 品牌名 */}
        <h1 className="mb-2 text-center text-2xl font-bold tracking-wider text-white md:text-3xl">
          唤星
          <span className="ml-2 text-lg font-light tracking-widest text-[#A5B4FC] opacity-70 md:text-xl">
            HUANXING
          </span>
        </h1>
        <p className="mb-8 text-center text-sm text-[#8b9fd8]">
          唤醒星辰的力量，AI与你共生
        </p>

        {/* Onboard 进度页 */}
        {step === "onboard" ? (
          <OnboardProgress steps={onboardSteps} nickname={loginNickname} />
        ) : (
        /* 登录卡片 */
        <div
          className="w-full max-w-sm overflow-hidden rounded-2xl border border-[#1e2d5a]/60 p-6 backdrop-blur-xl"
          style={{
            background:
              "linear-gradient(160deg, rgba(10,17,40,0.85), rgba(5,8,22,0.92))",
            boxShadow:
              "0 0 80px -20px rgba(124,58,237,0.15), inset 0 1px 0 rgba(165,180,252,0.08)",
          }}
        >
          {/* 顶部渐变线 */}
          <div className="absolute left-0 right-0 top-0 h-[2px] bg-gradient-to-r from-transparent via-[#7C3AED] to-transparent opacity-60" />

          {step === "phone" ? (
            <div className="space-y-4">
              <p className="text-center text-sm text-[#9bb8e8]">
                输入手机号登录
              </p>
              <div className="relative">
                <span className="absolute left-4 top-1/2 -translate-y-1/2 text-sm text-[#7C3AED]">
                  +86
                </span>
                <input
                  type="tel"
                  value={phone}
                  onChange={(e) =>
                    setPhone(e.target.value.replace(/\D/g, "").slice(0, 11))
                  }
                  onKeyDown={handleKeyDown}
                  placeholder="手机号"
                  className="w-full rounded-xl border border-[#1e2d5a] bg-[#0a1128]/80 pl-14 pr-4 py-3 text-base text-white placeholder-[#4a5f8f] focus:border-[#7C3AED] focus:outline-none focus:ring-1 focus:ring-[#7C3AED]/30 transition-all"
                  maxLength={11}
                  autoFocus
                />
              </div>
              {error && (
                <p className="text-center text-sm text-rose-400">{error}</p>
              )}
              <button
                type="button"
                onClick={sendCode}
                disabled={loading || phone.length !== 11}
                className="w-full rounded-xl py-3 text-sm font-semibold text-white transition-all duration-300 disabled:opacity-40 hover:brightness-110 hover:shadow-[0_0_24px_rgba(124,58,237,0.4)]"
                style={{
                  background:
                    "linear-gradient(135deg, #7C3AED, #4F46E5, #06B6D4)",
                }}
              >
                {loading ? "发送中..." : "获取验证码"}
              </button>
            </div>
          ) : (
            <div className="space-y-4">
              <p className="text-center text-sm text-[#9bb8e8]">
                验证码已发送到{" "}
                {phone.replace(/(\d{3})\d{4}(\d{4})/, "$1****$2")}
              </p>
              <input
                type="text"
                value={code}
                onChange={(e) =>
                  setCode(e.target.value.replace(/\D/g, "").slice(0, 6))
                }
                onKeyDown={handleKeyDown}
                placeholder="6位验证码"
                className="w-full rounded-xl border border-[#1e2d5a] bg-[#0a1128]/80 px-4 py-3 text-center text-2xl tracking-[0.4em] text-white placeholder-[#4a5f8f] focus:border-[#7C3AED] focus:outline-none focus:ring-1 focus:ring-[#7C3AED]/30 transition-all"
                maxLength={6}
                autoFocus
              />
              {error && (
                <p className="text-center text-sm text-rose-400">{error}</p>
              )}
              <button
                type="button"
                onClick={handleLogin}
                disabled={loading || code.length !== 6}
                className="w-full rounded-xl py-3 text-sm font-semibold text-white transition-all duration-300 disabled:opacity-40 hover:brightness-110 hover:shadow-[0_0_24px_rgba(124,58,237,0.4)]"
                style={{
                  background:
                    "linear-gradient(135deg, #7C3AED, #4F46E5, #06B6D4)",
                }}
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
                  className="text-[#8b9fd8] hover:text-white transition-colors"
                >
                  ← 换号码
                </button>
                <button
                  type="button"
                  onClick={sendCode}
                  disabled={countdown > 0}
                  className="text-[#8b9fd8] hover:text-white transition-colors disabled:opacity-40"
                >
                  {countdown > 0 ? `${countdown}s 后重发` : "重新发送"}
                </button>
              </div>
            </div>
          )}

          {/* 协议 */}
          <p className="mt-6 text-center text-xs text-[#4a5f8f]">
            登录即同意{" "}
            <span className="text-[#7C3AED]/70 cursor-pointer hover:text-[#A78BFA]">
              用户协议
            </span>{" "}
            和{" "}
            <span className="text-[#7C3AED]/70 cursor-pointer hover:text-[#A78BFA]">
              隐私政策
            </span>
          </p>
        </div>
        )}

        {/* 底部版权 */}
        <p className="mt-8 text-center text-xs text-[#3a4a6f]">
          © 2026 唤星AI · HASN Protocol
        </p>
      </div>
    </div>
  );
}
