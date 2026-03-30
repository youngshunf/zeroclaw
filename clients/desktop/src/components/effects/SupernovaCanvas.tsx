import React, { useEffect, useRef } from "react";
import { playSupernova } from "../../lib/audio";

export interface SupernovaProps {
  onDone: () => void;
}

export function SupernovaCanvas({ onDone }: SupernovaProps) {
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

    const brandColorHex = getComputedStyle(document.documentElement).getPropertyValue('--color-brand').trim() || '#7C3AED';

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
        grad.addColorStop(1, `${brandColorHex}00`);
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
          glow.addColorStop(1, `${brandColorHex}00`);
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
