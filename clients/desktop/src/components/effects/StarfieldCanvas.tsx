import React, { useEffect, useRef } from "react";

interface Star {
  x: number;
  y: number;
  r: number;
  baseAlpha: number;
  alpha: number;
  twinkleSpeed: number;
  twinklePhase: number;
  twinkleAmp: number;
  vx: number;
  vy: number;
}

interface Meteor {
  x: number;
  y: number;
  vx: number;
  vy: number;
  len: number;
  alpha: number;
  life: number;
  maxLife: number;
}

export function StarfieldCanvas() {
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
      const isBright = Math.random() < 0.15;
      stars.push({
        x: Math.random() * canvas.width,
        y: Math.random() * canvas.height,
        r: isBright ? 1.2 + Math.random() * 1.0 : 0.4 + Math.random() * 1.2,
        baseAlpha: isBright ? 0.5 + Math.random() * 0.5 : baseAlpha,
        alpha: baseAlpha,
        twinkleSpeed: isBright
          ? 0.015 + Math.random() * 0.025
          : 0.003 + Math.random() * 0.008,
        twinklePhase: Math.random() * Math.PI * 2,
        twinkleAmp: isBright ? 0.85 : 0.45,
        vx: (Math.random() - 0.5) * 0.06,
        vy: (Math.random() - 0.5) * 0.06,
      });
    }

    // ── 流星管理 ───────────────────────────────────
    const meteors: Meteor[] = [];
    let nextMeteorIn = 80 + Math.random() * 120;

    function spawnMeteor(w: number, h: number) {
      const angle = (Math.PI / 6) + Math.random() * (Math.PI / 8);
      const speed = 8 + Math.random() * 10;
      const len = 120 + Math.random() * 180;
      const maxLife = Math.floor((len / speed) * 2.5);
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

        const phase = time * s.twinkleSpeed + s.twinklePhase;
        const flicker = 1 - s.twinkleAmp + s.twinkleAmp * Math.abs(Math.sin(phase));
        s.alpha = s.baseAlpha * flicker;

        ctx.beginPath();
        ctx.arc(s.x, s.y, s.r, 0, Math.PI * 2);
        const c = s.r > 1.5 ? `rgba(230,230,255,${s.alpha})` : `rgba(180,195,255,${s.alpha})`;
        ctx.fillStyle = c;
        ctx.fill();

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
        for (let m = 0; m < 3; m++) {
          spawnMeteor(w, h);
        }
        nextMeteorIn = 90 + Math.random() * 150;
      }

      // ── 更新 & 绘制流星 ─────────────────────────
      for (let i = meteors.length - 1; i >= 0; i--) {
        const m = meteors[i];
        m.life++;
        m.x += m.vx;
        m.y += m.vy;

        const progress = m.life / m.maxLife;
        const fadeAlpha = progress < 0.5
          ? m.alpha
          : m.alpha * (1 - (progress - 0.5) * 2);

        if (fadeAlpha <= 0 || m.life >= m.maxLife || m.y > h + 50) {
          meteors.splice(i, 1);
          continue;
        }

        const speed = Math.sqrt(m.vx * m.vx + m.vy * m.vy);
        const nx = -m.vx / speed;
        const ny = -m.vy / speed;
        const tailLen = m.len * Math.min(1, progress * 3);

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
