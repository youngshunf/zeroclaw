/**
 * useVoiceRecorder — 麦克风录音 Hook
 *
 * 使用 Web API (getUserMedia + MediaRecorder) 实现录音，
 * 输出 WebM/Opus 格式的音频 Blob，可直接上传给 STT API。
 *
 * 功能：
 * - 请求麦克风权限
 * - 开始/停止/取消录音
 * - 录音时长计时
 * - 音量指示 (RMS)
 * - 错误处理
 */
import { useState, useRef, useCallback, useEffect } from 'react';

export type VoiceRecorderState = 'idle' | 'requesting' | 'recording' | 'processing' | 'error';

export interface UseVoiceRecorderOptions {
  /** 最大录音时长（秒），默认 120 */
  maxDurationSecs?: number;
  /** 录音完成回调 (audio blob, duration) */
  onRecordingComplete?: (blob: Blob, durationSecs: number) => void;
  /** 错误回调 */
  onError?: (error: string) => void;
}

export interface UseVoiceRecorderReturn {
  /** 当前状态 */
  state: VoiceRecorderState;
  /** 录音时长（秒） */
  duration: number;
  /** 音量级别 (0~1) */
  volume: number;
  /** 错误消息 */
  errorMessage: string | null;
  /** 开始录音 */
  startRecording: () => Promise<void>;
  /** 停止录音并返回 Blob */
  stopRecording: () => void;
  /** 取消录音 */
  cancelRecording: () => void;
  /** 是否支持录音 */
  isSupported: boolean;
}

export function useVoiceRecorder(options: UseVoiceRecorderOptions = {}): UseVoiceRecorderReturn {
  const {
    maxDurationSecs = 120,
    onRecordingComplete,
    onError,
  } = options;

  const [state, setState] = useState<VoiceRecorderState>('idle');
  const [duration, setDuration] = useState(0);
  const [volume, setVolume] = useState(0);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const timerRef = useRef<number | null>(null);
  const maxTimerRef = useRef<number | null>(null);
  const startTimeRef = useRef<number>(0);

  // Audio analysis for volume meter
  const analyserRef = useRef<AnalyserNode | null>(null);
  const audioContextRef = useRef<AudioContext | null>(null);
  const animFrameRef = useRef<number | null>(null);

  const isSupported = typeof navigator !== 'undefined'
    && !!navigator.mediaDevices
    && !!navigator.mediaDevices.getUserMedia
    && typeof MediaRecorder !== 'undefined';

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      cleanup();
    };
  }, []);

  const cleanup = useCallback(() => {
    if (timerRef.current) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
    if (maxTimerRef.current) {
      clearTimeout(maxTimerRef.current);
      maxTimerRef.current = null;
    }
    if (animFrameRef.current) {
      cancelAnimationFrame(animFrameRef.current);
      animFrameRef.current = null;
    }
    if (audioContextRef.current) {
      audioContextRef.current.close().catch(() => {});
      audioContextRef.current = null;
    }
    if (streamRef.current) {
      streamRef.current.getTracks().forEach(track => track.stop());
      streamRef.current = null;
    }
    mediaRecorderRef.current = null;
    analyserRef.current = null;
    chunksRef.current = [];
  }, []);

  // Volume analysis loop
  const analyzeVolume = useCallback(() => {
    if (!analyserRef.current) return;

    const analyser = analyserRef.current;
    const dataArray = new Uint8Array(analyser.fftSize);
    analyser.getByteTimeDomainData(dataArray);

    // Calculate RMS
    let sum = 0;
    for (let i = 0; i < dataArray.length; i++) {
      const normalized = (dataArray[i] - 128) / 128;
      sum += normalized * normalized;
    }
    const rms = Math.sqrt(sum / dataArray.length);
    setVolume(Math.min(1, rms * 3)); // Amplify for visual feedback

    animFrameRef.current = requestAnimationFrame(analyzeVolume);
  }, []);

  const startRecording = useCallback(async () => {
    if (!isSupported) {
      const msg = '您的浏览器不支持录音功能';
      setErrorMessage(msg);
      setState('error');
      onError?.(msg);
      return;
    }

    try {
      setState('requesting');
      setErrorMessage(null);
      setDuration(0);
      setVolume(0);
      chunksRef.current = [];

      const stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          echoCancellation: true,
          noiseSuppression: true,
          autoGainControl: true,
        },
      });

      streamRef.current = stream;

      // Setup audio analysis for volume meter
      const audioContext = new AudioContext();
      audioContextRef.current = audioContext;
      const source = audioContext.createMediaStreamSource(stream);
      const analyser = audioContext.createAnalyser();
      analyser.fftSize = 256;
      source.connect(analyser);
      analyserRef.current = analyser;

      // Determine best MIME type
      const mimeType = getSupportedMimeType();

      const recorder = new MediaRecorder(stream, {
        mimeType,
        audioBitsPerSecond: 64000,
      });

      mediaRecorderRef.current = recorder;

      recorder.ondataavailable = (event) => {
        if (event.data.size > 0) {
          chunksRef.current.push(event.data);
        }
      };

      recorder.onstop = () => {
        const blob = new Blob(chunksRef.current, { type: mimeType });
        const durationSecs = Math.round((Date.now() - startTimeRef.current) / 1000);

        if (blob.size > 0 && state !== 'idle') {
          onRecordingComplete?.(blob, durationSecs);
        }

        setState('idle');
        cleanup();
      };

      recorder.onerror = () => {
        const msg = '录音出错，请重试';
        setErrorMessage(msg);
        setState('error');
        onError?.(msg);
        cleanup();
      };

      // Start recording with 250ms timeslice for streaming chunks
      recorder.start(250);
      startTimeRef.current = Date.now();
      setState('recording');

      // Start duration timer
      timerRef.current = window.setInterval(() => {
        setDuration(Math.round((Date.now() - startTimeRef.current) / 1000));
      }, 500);

      // Start volume analysis
      analyzeVolume();

      // Auto-stop at max duration
      maxTimerRef.current = window.setTimeout(() => {
        if (mediaRecorderRef.current?.state === 'recording') {
          mediaRecorderRef.current.stop();
        }
      }, maxDurationSecs * 1000);

    } catch (error: any) {
      let msg = '无法访问麦克风';
      if (error?.name === 'NotAllowedError' || error?.name === 'PermissionDeniedError') {
        msg = '麦克风权限被拒绝，请在系统设置中允许';
      } else if (error?.name === 'NotFoundError') {
        msg = '未找到麦克风设备';
      } else if (error?.name === 'NotReadableError') {
        msg = '麦克风被其他应用占用';
      }
      setErrorMessage(msg);
      setState('error');
      onError?.(msg);
      cleanup();
    }
  }, [isSupported, maxDurationSecs, onRecordingComplete, onError, analyzeVolume, cleanup]);

  const stopRecording = useCallback(() => {
    if (mediaRecorderRef.current?.state === 'recording') {
      setState('processing');
      mediaRecorderRef.current.stop();
    }
  }, []);

  const cancelRecording = useCallback(() => {
    // Set state to idle BEFORE stopping — prevents onstop from firing callback
    setState('idle');
    if (mediaRecorderRef.current?.state === 'recording') {
      mediaRecorderRef.current.stop();
    }
    cleanup();
    setDuration(0);
    setVolume(0);
  }, [cleanup]);

  return {
    state,
    duration,
    volume,
    errorMessage,
    startRecording,
    stopRecording,
    cancelRecording,
    isSupported,
  };
}

// ── Helpers ──────────────────────────────────────────────────────

/** Get the best supported audio MIME type for MediaRecorder */
function getSupportedMimeType(): string {
  // Prefer WebM/Opus — supported by both WebKit and Chromium
  const types = [
    'audio/webm;codecs=opus',
    'audio/webm',
    'audio/ogg;codecs=opus',
    'audio/ogg',
    'audio/mp4',
  ];

  for (const type of types) {
    if (MediaRecorder.isTypeSupported(type)) {
      return type;
    }
  }

  // Last resort
  return '';
}

/** Format seconds to mm:ss */
export function formatDuration(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${s.toString().padStart(2, '0')}`;
}

/**
 * Transcribe audio blob via ZeroClaw sidecar API
 *
 * POST /api/audio/transcribe (multipart/form-data)
 * Response: { text: string }
 */
export async function transcribeAudio(blob: Blob): Promise<string> {
  const formData = new FormData();

  // Determine proper filename from MIME
  const ext = blob.type.includes('webm') ? 'webm'
    : blob.type.includes('ogg') ? 'ogg'
    : blob.type.includes('mp4') ? 'm4a'
    : 'webm';

  formData.append('file', blob, `voice.${ext}`);

  const { getToken } = await import('@/lib/auth');
  const token = getToken();

  const resp = await fetch('/api/audio/transcribe', {
    method: 'POST',
    headers: token ? { 'Authorization': `Bearer ${token}` } : {},
    body: formData,
  });

  if (!resp.ok) {
    const errorText = await resp.text().catch(() => '');
    throw new Error(`STT failed (${resp.status}): ${errorText || resp.statusText}`);
  }

  const data = await resp.json();
  return (data.text || '').trim();
}
