/**
 * 超新星爆炸音效（Web Audio API 合成）
 */
export function playSupernova() {
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

    // 两次脉冲
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
