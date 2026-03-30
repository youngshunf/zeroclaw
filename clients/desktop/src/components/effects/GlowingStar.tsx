import React from "react";
import starSvg from "@/assets/huanxing-star.svg";

export function GlowingStar() {
  return (
    <div className="relative mx-auto mb-6 flex h-52 w-52 items-center justify-center md:h-64 md:w-64">
      {/* 最外层超大脉冲光晕 */}
      <div
        className="absolute rounded-full animate-pulse-slow"
        style={{
          width: "200%",
          height: "200%",
          background:
            "radial-gradient(circle, color-mix(in srgb, var(--color-brand) 18%, transparent) 0%, color-mix(in srgb, var(--color-brand-light) 8%, transparent) 40%, transparent 70%)",
        }}
      />
      {/* 最外层光晕 */}
      <div
        className="absolute h-full w-full rounded-full animate-pulse-slow inset-0"
        style={{
          background:
            "radial-gradient(circle, color-mix(in srgb, var(--color-brand) 35%, transparent) 0%, color-mix(in srgb, var(--color-brand-light) 15%, transparent) 45%, transparent 72%)",
        }}
      />
      {/* 中层暖光圈 */}
      <div
        className="absolute h-4/5 w-4/5 rounded-full animate-pulse-medium"
        style={{
          background:
            "radial-gradient(circle, rgba(255,217,61,0.2) 0%, color-mix(in srgb, var(--color-brand) 20%, transparent) 50%, transparent 70%)",
        }}
      />
      {/* SVG 星芒 Logo */}
      <img
        src={starSvg}
        alt="唤星"
        className="relative h-32 w-32 md:h-40 md:w-40"
        style={{
          filter:
            "drop-shadow(0 0 30px color-mix(in srgb, var(--color-brand) 80%, transparent)) drop-shadow(0 0 80px color-mix(in srgb, var(--color-brand-light) 50%, transparent)) drop-shadow(0 0 120px color-mix(in srgb, var(--color-brand) 30%, transparent))",
        }}
      />
      {/* 中心白点 */}
      <div
        className="absolute h-4 w-4 rounded-full bg-white animate-pulse-fast"
        style={{
          boxShadow:
            "0 0 16px rgba(255,255,255,0.9), 0 0 60px color-mix(in srgb, var(--color-brand) 70%, transparent), 0 0 120px color-mix(in srgb, var(--color-brand-light) 40%, transparent)",
        }}
      />
    </div>
  );
}
