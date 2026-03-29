/**
 * 唤星登录页 — 星空连线 + 发光星芒 + 手机号验证码登录
 *
 * 设计参考唤星官网 Hero 风格：
 * - 深空背景 + 随机星点
 * - 星星之间动态连线（Canvas）
 * - 中央发光八角星芒（品牌 Logo）
 * - 渐变登录卡片
 */
import React from "react";
import { useState, useEffect, useCallback, useRef } from "react";
import starSvg from "../assets/huanxing-star.svg";
import { saveHuanxingSession, type HuanxingLoginData } from "../config";
import { autoOnboard, registerHasnIdentity, registerHasnAgent, connectHasn } from "../onboard";
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
  // 部分亮星有闪烁幅度放大效果
  twinkleAmp: number;
  vx: number;
  vy: number;
}

interface Meteor {
  x: number;
  y: number;
  vx: number;
  vy: number;
  len: number;   // 尾迹长度（px）
  alpha: number;
  life: number;
  maxLife: number;
}

function StarfieldCanvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animRef = useRef<number>(0);

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

    // ── 初始化星星 ──────────────────────────────────
    const count = Math.floor((canvas.width * canvas.height) / 5500);
    const stars: Star[] = [];
    for (let i = 0; i < count; i++) {
      const baseAlpha = 0.2 + Math.random() * 0.65;
      // 约 15% 的星星是"亮星"，闪烁幅度更大
      const isBright = Math.random() < 0.15;
      stars.push({
        x: Math.random() * canvas.width,
        y: Math.random() * canvas.height,
        r: isBright ? 1.2 + Math.random() * 1.0 : 0.4 + Math.random() * 1.2,
        baseAlpha: isBright ? 0.5 + Math.random() * 0.5 : baseAlpha,
        alpha: baseAlpha,
        twinkleSpeed: isBright
          ? 0.015 + Math.random() * 0.025   // 亮星闪烁快
          : 0.003 + Math.random() * 0.008,
        twinklePhase: Math.random() * Math.PI * 2,
        twinkleAmp: isBright ? 0.85 : 0.45,
        vx: (Math.random() - 0.5) * 0.06,
        vy: (Math.random() - 0.5) * 0.06,
      });
    }

    // ── 流星管理 ───────────────────────────────────
    const meteors: Meteor[] = [];
    let nextMeteorIn = 80 + Math.random() * 120; // 帧数间隔

    function spawnMeteor(w: number, h: number) {
      // 从顶部 / 右上侧随机位置出发，向左下划过
      const angle = (Math.PI / 6) + Math.random() * (Math.PI / 8); // 约 30°-52.5°
      const speed = 8 + Math.random() * 10;
      const len = 120 + Math.random() * 180;
      const maxLife = Math.floor(len / speed * 2.5);
      meteors.push({
        x: Math.random() * w * 0.9 + w * 0.1,
        y: -20,
        vx: -Math.cos(angle) * speed,
        vy: Math.sin(angle) * speed,
        len,
        alpha: 0.9 + Math.random() * 0.1,
        life: 0,
        maxLife,
      });
    }

    const CONNECTION_DIST = 120;
    let time = 0;

    const draw = () => {
      time++;
      const w = canvas.width;
      const h = canvas.height;
      ctx.clearRect(0, 0, w, h);

      // ── 更新 & 绘制星星 ──────────────────────────
      for (const s of stars) {
        s.x += s.vx;
        s.y += s.vy;
        if (s.x < -10) s.x = w + 10;
        if (s.x > w + 10) s.x = -10;
        if (s.y < -10) s.y = h + 10;
        if (s.y > h + 10) s.y = -10;

        // 闪烁：正弦波 + 偶发"脉冲"（用 abs(sin) 模拟快速点亮）
        const phase = time * s.twinkleSpeed + s.twinklePhase;
        const flicker = 1 - s.twinkleAmp + s.twinkleAmp * Math.abs(Math.sin(phase));
        s.alpha = s.baseAlpha * flicker;

        ctx.beginPath();
        ctx.arc(s.x, s.y, s.r, 0, Math.PI * 2);
        // 亮星用偏暖白色，普通星偏蓝紫
        const c = s.r > 1.5 ? `rgba(230,230,255,${s.alpha})` : `rgba(180,195,255,${s.alpha})`;
        ctx.fillStyle = c;
        ctx.fill();

        // 亮星额外发出微光晕
        if (s.r > 1.5 && s.alpha > 0.6) {
          const grd = ctx.createRadialGradient(s.x, s.y, 0, s.x, s.y, s.r * 4);
          grd.addColorStop(0, `rgba(200,180,255,${s.alpha * 0.4})`);
          grd.addColorStop(1, "rgba(0,0,0,0)");
          ctx.beginPath();
          ctx.arc(s.x, s.y, s.r * 4, 0, Math.PI * 2);
          ctx.fillStyle = grd;
          ctx.fill();
        }
      }

      // ── 绘制连线 ────────────────────────────────
      for (let i = 0; i < stars.length; i++) {
        for (let j = i + 1; j < stars.length; j++) {
          const dx = stars[i].x - stars[j].x;
          const dy = stars[i].y - stars[j].y;
          const dist = Math.sqrt(dx * dx + dy * dy);
          if (dist < CONNECTION_DIST) {
            const lineAlpha =
              (1 - dist / CONNECTION_DIST) *
              0.12 *
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

      // ── 生成新流星 ──────────────────────────────
      nextMeteorIn--;
      if (nextMeteorIn <= 0) {
        // 每批同时生成 3 颗，间距随机错开，制造群星效果
        for (let m = 0; m < 3; m++) {
          spawnMeteor(w, h);
        }
        nextMeteorIn = 90 + Math.random() * 150; // 1.5s ~ 4s @60fps
      }

      // ── 更新 & 绘制流星 ─────────────────────────
      for (let i = meteors.length - 1; i >= 0; i--) {
        const m = meteors[i];
        m.life++;
        m.x += m.vx;
        m.y += m.vy;

        // 生命周期淡出（后半段）
        const progress = m.life / m.maxLife;
        const fadeAlpha = progress < 0.5
          ? m.alpha
          : m.alpha * (1 - (progress - 0.5) * 2);

        if (fadeAlpha <= 0 || m.life >= m.maxLife || m.y > h + 50) {
          meteors.splice(i, 1);
          continue;
        }

        // 尾迹方向（反速度方向）
        const speed = Math.sqrt(m.vx * m.vx + m.vy * m.vy);
        const nx = -m.vx / speed;
        const ny = -m.vy / speed;
        const tailLen = m.len * Math.min(1, progress * 3); // 先短后长

        const grd = ctx.createLinearGradient(
          m.x, m.y,
          m.x + nx * tailLen, m.y + ny * tailLen
        );
        grd.addColorStop(0, `rgba(255,255,255,${fadeAlpha})`);
        grd.addColorStop(0.15, `rgba(180,160,255,${fadeAlpha * 0.7})`);
        grd.addColorStop(1, "rgba(100,80,200,0)");

        ctx.beginPath();
        ctx.moveTo(m.x, m.y);
        ctx.lineTo(m.x + nx * tailLen, m.y + ny * tailLen);
        ctx.strokeStyle = grd;
        ctx.lineWidth = 1.5;
        ctx.lineCap = "round";
        ctx.stroke();

        // 流星头部亮点
        const headGrd = ctx.createRadialGradient(m.x, m.y, 0, m.x, m.y, 4);
        headGrd.addColorStop(0, `rgba(255,255,255,${fadeAlpha})`);
        headGrd.addColorStop(1, "rgba(255,255,255,0)");
        ctx.beginPath();
        ctx.arc(m.x, m.y, 4, 0, Math.PI * 2);
        ctx.fillStyle = headGrd;
        ctx.fill();
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

// ─── 超新星爆炸音效（Web Audio API 合成）────────────────────────────
function playSupernova() {
  try {
    const ctx = new AudioContext();
    const now = ctx.currentTime;

    // 低频冲击 boom：瞬间爆发，快速衰减
    const boom = ctx.createOscillator();
    const boomGain = ctx.createGain();
    boom.type = "sine";
    boom.frequency.setValueAtTime(80, now);
    boom.frequency.exponentialRampToValueAtTime(20, now + 1.5);
    boomGain.gain.setValueAtTime(0, now);
    boomGain.gain.linearRampToValueAtTime(1.0, now + 0.04);
    boomGain.gain.exponentialRampToValueAtTime(0.001, now + 1.5);
    boom.connect(boomGain);
    boomGain.connect(ctx.destination);
    boom.start(now);
    boom.stop(now + 1.5);

    // 低频噪声体：给 boom 加厚度
    const bufSize = ctx.sampleRate * 2;
    const noiseBuf = ctx.createBuffer(1, bufSize, ctx.sampleRate);
    const nd = noiseBuf.getChannelData(0);
    for (let i = 0; i < bufSize; i++) nd[i] = Math.random() * 2 - 1;
    const noiseBody = ctx.createBufferSource();
    noiseBody.buffer = noiseBuf;
    const bodyFilter = ctx.createBiquadFilter();
    bodyFilter.type = "lowpass";
    bodyFilter.frequency.setValueAtTime(200, now);
    bodyFilter.frequency.exponentialRampToValueAtTime(40, now + 1.5);
    const bodyGain = ctx.createGain();
    bodyGain.gain.setValueAtTime(0, now);
    bodyGain.gain.linearRampToValueAtTime(0.6, now + 0.05);
    bodyGain.gain.exponentialRampToValueAtTime(0.001, now + 1.5);
    noiseBody.connect(bodyFilter);
    bodyFilter.connect(bodyGain);
    bodyGain.connect(ctx.destination);
    noiseBody.start(now);

    // 持续余震 rumble：贯穿整个 3s
    const rumbleBufSize = ctx.sampleRate * 3.5;
    const rumbleBuf = ctx.createBuffer(1, rumbleBufSize, ctx.sampleRate);
    const rd = rumbleBuf.getChannelData(0);
    for (let i = 0; i < rumbleBufSize; i++) rd[i] = Math.random() * 2 - 1;
    const rumbleNoise = ctx.createBufferSource();
    rumbleNoise.buffer = rumbleBuf;
    const rumbleFilter = ctx.createBiquadFilter();
    rumbleFilter.type = "lowpass";
    rumbleFilter.frequency.setValueAtTime(120, now);
    rumbleFilter.frequency.exponentialRampToValueAtTime(30, now + 3.0);
    const rumbleGain = ctx.createGain();
    rumbleGain.gain.setValueAtTime(0, now);
    rumbleGain.gain.linearRampToValueAtTime(0.35, now + 0.2);
    rumbleGain.gain.setValueAtTime(0.3, now + 1.0);
    rumbleGain.gain.exponentialRampToValueAtTime(0.001, now + 3.0);
    rumbleNoise.connect(rumbleFilter);
    rumbleFilter.connect(rumbleGain);
    rumbleGain.connect(ctx.destination);
    rumbleNoise.start(now);

    // 两次脉冲（比原来少）
    for (let i = 1; i <= 2; i++) {
      const pulseTime = now + i * 0.6;
      const pulse = ctx.createOscillator();
      const pulseGain = ctx.createGain();
      pulse.type = "sine";
      pulse.frequency.setValueAtTime(55 - i * 8, pulseTime);
      pulse.frequency.exponentialRampToValueAtTime(15, pulseTime + 0.4);
      pulseGain.gain.setValueAtTime(0, pulseTime);
      pulseGain.gain.linearRampToValueAtTime(0.3 - i * 0.08, pulseTime + 0.03);
      pulseGain.gain.exponentialRampToValueAtTime(0.001, pulseTime + 0.4);
      pulse.connect(pulseGain);
      pulseGain.connect(ctx.destination);
      pulse.start(pulseTime);
      pulse.stop(pulseTime + 0.4);
    }

    setTimeout(() => ctx.close(), 4000);
  } catch {
    // 静默失败
  }
}

// ─── 超新星爆炸 Canvas ───────────────────────────────────────────────
// 新流程：
//   Phase 1: 爆炸扩散 (0 ~ EXPLOSION_END ≈ 2s) — 粒子从中心飞射并停留
//   Phase 2: onDone 回调 → 登录页开始淡入，粒子在画面上闪烁停留
//   Phase 3: 粒子开始慢慢消融 (PARTICLE_FADE_MS ≈ 1.5s)，canvas 最终移除
interface SupernovaProps {
  onDone: () => void;
}

function SupernovaCanvas({ onDone }: SupernovaProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animRef = useRef<number>(0);
  const onDoneRef = useRef(onDone);
  onDoneRef.current = onDone;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    canvas.width = window.innerWidth;
    canvas.height = window.innerHeight;
    const cx = canvas.width / 2;
    const cy = canvas.height / 2;

    playSupernova();

    // ── 时间节点 ──
    const EXPLOSION_END = 2000;     // 2s 爆炸扩散阶段
    const STOP_EMIT     = 1600;     // 1.6s 后停止喷射新粒子
    const PARTICLE_FADE_MS = 1800;  // 粒子消融持续 1.8s

    const COLORS = [
      "#ffffff", "#ffe066", "#ffd700",
      "#c084fc", "#818cf8", "#67e8f9",
      "#f472b6", "#fb923c", "#a5f3fc",
    ];

    // ── 粒子定义 ──
    interface Particle {
      x: number; y: number;
      vx: number; vy: number;
      r: number;
      alpha: number;
      color: string;
      spawnTime: number;
      travelMs: number;   // 飞行时间，之后停在原地
      settled: boolean;    // 是否已到达目的地
    }

    const maxDim = Math.max(canvas.width, canvas.height);
    const particles: Particle[] = [];

    function spawnBurst(spawnTime: number, count: number) {
      for (let i = 0; i < count; i++) {
        const angle = Math.random() * Math.PI * 2;
        const dist = maxDim * (0.2 + Math.random() * 0.65);
        const travelMs = 300 + Math.random() * 600;           // 更快飞到位
        const speed = dist / (travelMs / 16.67);
        particles.push({
          x: cx, y: cy,
          vx: Math.cos(angle) * speed,
          vy: Math.sin(angle) * speed,
          r: 0.8 + Math.random() * 2.5,
          alpha: 1.0,
          color: COLORS[Math.floor(Math.random() * COLORS.length)],
          spawnTime,
          travelMs,
          settled: false,
        });
      }
    }

    spawnBurst(0, 200); // 初始爆炸大批

    // ── 冲击波环 ──
    interface Ring { speed: number; maxR: number; width: number; color: string; interval: number; lastSpawn: number; }
    const ringDefs: Ring[] = [
      { speed: 6, maxR: maxDim * 0.85, width: 3, color: "180,140,255", interval: 600,  lastSpawn: -999 },
      { speed: 5, maxR: maxDim * 0.7,  width: 2, color: "103,232,249", interval: 800,  lastSpawn: -300 },
      { speed: 4, maxR: maxDim * 0.55, width: 1, color: "255,224,102", interval: 1000, lastSpawn: -600 },
    ];
    interface RingInstance { r: number; alpha: number; defIdx: number; }
    const ringInstances: RingInstance[] = [];

    let startTime: number | null = null;
    let lastEmit = 0;
    let doneFired = false;
    let fadeStartTime: number | null = null; // 粒子开始消融的时刻

    const draw = (ts: number) => {
      if (!startTime) startTime = ts;
      const elapsed = ts - startTime;

      // ── 喷射阶段 ──
      if (elapsed < STOP_EMIT && elapsed - lastEmit > 120) {
        spawnBurst(elapsed, 25 + Math.floor(Math.random() * 10));
        lastEmit = elapsed;
      }

      ctx.clearRect(0, 0, canvas.width, canvas.height);

      // ── 核心闪光（前 0.4s）──
      if (elapsed < 400) {
        const ft = elapsed / 400;
        const flashAlpha = ft < 0.15 ? ft / 0.15 : 1 - (ft - 0.15) / 0.85;
        const flashR = 10 + ft * 200;
        const grad = ctx.createRadialGradient(cx, cy, 0, cx, cy, flashR);
        grad.addColorStop(0, `rgba(255,255,255,${flashAlpha * 0.98})`);
        grad.addColorStop(0.3, `rgba(220,200,255,${flashAlpha * 0.6})`);
        grad.addColorStop(1, `rgba(124,58,237,0)`);
        ctx.fillStyle = grad;
        ctx.beginPath();
        ctx.arc(cx, cy, flashR, 0, Math.PI * 2);
        ctx.fill();
        if (elapsed < 60) {
          ctx.fillStyle = `rgba(255,255,255,${(1 - elapsed / 60) * 0.85})`;
          ctx.fillRect(0, 0, canvas.width, canvas.height);
        }
      }

      // ── 冲击波环 ──
      for (let i = 0; i < ringDefs.length; i++) {
        const def = ringDefs[i];
        if (elapsed < STOP_EMIT && elapsed - def.lastSpawn > def.interval) {
          ringInstances.push({ r: 2, alpha: 1.0, defIdx: i });
          def.lastSpawn = elapsed;
        }
      }
      for (let i = ringInstances.length - 1; i >= 0; i--) {
        const inst = ringInstances[i];
        const def = ringDefs[inst.defIdx];
        inst.r += def.speed;
        inst.alpha = Math.max(0, 1 - inst.r / def.maxR);
        if (inst.r >= def.maxR || inst.alpha < 0.01) {
          ringInstances.splice(i, 1);
          continue;
        }
        ctx.beginPath();
        ctx.arc(cx, cy, inst.r, 0, Math.PI * 2);
        ctx.strokeStyle = `rgba(${def.color},${inst.alpha * 0.35})`;
        ctx.lineWidth = def.width * (0.3 + inst.alpha * 0.7);
        ctx.stroke();
      }

      // ── 粒子消融系数 ──
      // 爆炸结束后开始消融，消融进度 0→1
      let globalFade = 1.0;
      if (fadeStartTime !== null) {
        globalFade = Math.max(0, 1 - (ts - fadeStartTime) / PARTICLE_FADE_MS);
      }

      // ── 绘制粒子 ──
      let anyAlive = false;
      for (const p of particles) {
        const age = elapsed - p.spawnTime;
        if (age < 0) continue;

        // 飞行阶段
        if (!p.settled && age < p.travelMs) {
          p.x += p.vx;
          p.y += p.vy;
        } else {
          p.settled = true;
        }

        // 飞行过程中淡入，到达后保持1.0
        const arrivalAlpha = age < p.travelMs * 0.3
          ? age / (p.travelMs * 0.3)
          : 1.0;

        // 停留时轻微闪烁
        const twinkle = p.settled
          ? 0.6 + 0.4 * Math.sin(elapsed * 0.005 + p.spawnTime * 0.013)
          : 1.0;

        p.alpha = arrivalAlpha * twinkle * globalFade;
        if (p.alpha <= 0.005) continue;
        anyAlive = true;

        const hex = p.color;
        const r = parseInt(hex.slice(1, 3), 16);
        const g = parseInt(hex.slice(3, 5), 16);
        const b = parseInt(hex.slice(5, 7), 16);

        ctx.beginPath();
        ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2);
        ctx.fillStyle = `rgba(${r},${g},${b},${p.alpha})`;
        ctx.shadowBlur = 6;
        ctx.shadowColor = `rgba(${r},${g},${b},${p.alpha * 0.6})`;
        ctx.fill();
        ctx.shadowBlur = 0;
      }

      // ── 中心余辉（仅爆炸阶段）──
      if (elapsed < EXPLOSION_END) {
        const t = elapsed / EXPLOSION_END;
        const glowAlpha = Math.max(0, 0.35 * (1 - t * 1.5));
        if (glowAlpha > 0.01) {
          const glowR = 50 + t * 60;
          const glow = ctx.createRadialGradient(cx, cy, 0, cx, cy, glowR);
          glow.addColorStop(0, `rgba(200,160,255,${glowAlpha})`);
          glow.addColorStop(1, `rgba(124,58,237,0)`);
          ctx.fillStyle = glow;
          ctx.beginPath();
          ctx.arc(cx, cy, glowR, 0, Math.PI * 2);
          ctx.fill();
        }
      }

      // ── 状态转移 ──
      if (elapsed >= EXPLOSION_END && !doneFired) {
        // 爆炸结束 → 通知父组件淡入登录内容
        doneFired = true;
        onDoneRef.current();
        // 延迟 300ms 后开始让粒子消融，给登录页淡入一点缓冲
        setTimeout(() => { fadeStartTime = performance.now(); }, 300);
      }

      // 当还有活跃粒子或消融还没开始时，继续动画
      if (anyAlive || fadeStartTime === null) {
        animRef.current = requestAnimationFrame(draw);
      }
      // 否则所有粒子都消失了，canvas 自然停止（父组件会移除它）
    };

    animRef.current = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(animRef.current);
  }, []);

  return (
    <canvas
      ref={canvasRef}
      className="fixed inset-0 pointer-events-none"
      style={{ zIndex: 100 }}
    />
  );
}

// ─── 发光星芒组件 ────────────────────────────────────────────────────
function GlowingStar() {
  return (
    <div className="relative mx-auto mb-6 flex h-52 w-52 items-center justify-center md:h-64 md:w-64">
      {/* 最外层超大脉冲光晕 */}
      <div
        className="absolute rounded-full animate-pulse-slow"
        style={{
          width: "200%",
          height: "200%",
          background:
            "radial-gradient(circle, rgba(124,58,237,0.18) 0%, rgba(79,70,229,0.08) 40%, transparent 70%)",
        }}
      />
      {/* 最外层光晕 */}
      <div
        className="absolute h-full w-full rounded-full animate-pulse-slow"
        style={{
          background:
            "radial-gradient(circle, rgba(124,58,237,0.35) 0%, rgba(79,70,229,0.15) 45%, transparent 72%)",
        }}
      />
      {/* 中层暖光圈 */}
      <div
        className="absolute h-4/5 w-4/5 rounded-full animate-pulse-medium"
        style={{
          background:
            "radial-gradient(circle, rgba(255,217,61,0.2) 0%, rgba(124,58,237,0.2) 50%, transparent 70%)",
        }}
      />
      {/* SVG 星芒 Logo */}
      <img
        src={starSvg}
        alt="唤星"
        className="relative h-32 w-32 md:h-40 md:w-40"
        style={{
          filter:
            "drop-shadow(0 0 30px rgba(124,58,237,0.8)) drop-shadow(0 0 80px rgba(79,70,229,0.5)) drop-shadow(0 0 120px rgba(124,58,237,0.3))",
        }}
      />
      {/* 中心白点 */}
      <div
        className="absolute h-4 w-4 rounded-full bg-white animate-pulse-fast"
        style={{
          boxShadow:
            "0 0 16px rgba(255,255,255,0.9), 0 0 60px rgba(124,58,237,0.7), 0 0 120px rgba(79,70,229,0.4)",
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
    <div className="relative min-h-screen overflow-hidden bg-[#050816]">
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
      <div
        className="transition-opacity duration-1000 ease-out"
        style={{ opacity: contentVisible ? 1 : 0 }}
      >
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
      <div className="relative flex min-h-screen flex-col items-center justify-center px-4 pt-7" style={{ zIndex: 2 }}>
        {/* 发光星芒 */}
        <GlowingStar />

        {/* 品牌名 */}
        <h1 className="mb-2 text-center text-2xl font-bold tracking-wider text-white md:text-3xl">
          唤星AI
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
        <p className="mt-1 text-center text-[10px] text-[#2a3450]">
          OpenClaw 生态产品
        </p>
      </div>
      </div>{/* 主内容包裹结束 */}
    </div>
  );
}
