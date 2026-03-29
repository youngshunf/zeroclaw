/**
 * HxVoiceButton — 语音录音按钮组件
 *
 * 放在输入框左侧工具栏中（和 📎 / @ 并排）。
 *
 * 三种状态：
 * 1. idle        — 显示麦克风图标按钮
 * 2. recording   — 整个输入框区域被替换为录音条（通过 overlay）
 * 3. transcribing — 显示转录中状态
 */
import React, { useCallback, useState } from 'react';
import { Mic, MicOff, X, Send, Loader2 } from 'lucide-react';
import { useVoiceRecorder, formatDuration, transcribeAudio } from '@/huanxing/hooks/useVoiceRecorder';

export interface HxVoiceButtonProps {
  /** 转录完成回调 — 将文字填入输入框 */
  onTranscribed: (text: string) => void;
  /** 是否禁用（例如 generating 中） */
  disabled?: boolean;
}

export function HxVoiceButton({ onTranscribed, disabled }: HxVoiceButtonProps) {
  const [isTranscribing, setIsTranscribing] = useState(false);

  const handleRecordingComplete = useCallback(async (blob: Blob, _durationSecs: number) => {
    setIsTranscribing(true);
    try {
      const text = await transcribeAudio(blob);
      if (text) {
        onTranscribed(text);
      }
    } catch (err: any) {
      console.error('STT failed:', err);
    } finally {
      setIsTranscribing(false);
    }
  }, [onTranscribed]);

  const {
    state,
    duration,
    volume,
    errorMessage,
    startRecording,
    stopRecording,
    cancelRecording,
    isSupported,
  } = useVoiceRecorder({
    maxDurationSecs: 120,
    onRecordingComplete: handleRecordingComplete,
  });

  // If transcribing, show spinner inline in toolbar
  if (isTranscribing) {
    return (
      <div className="hx-voice-transcribing">
        <Loader2 size={16} className="hx-voice-spinner" />
        <span className="hx-voice-transcribing-text">识别中...</span>
      </div>
    );
  }

  // Recording mode — show full-width overlay bar
  if (state === 'recording') {
    return (
      <div className="hx-voice-recording-bar">
        {/* Cancel button */}
        <button
          type="button"
          className="hx-voice-cancel-btn"
          onClick={cancelRecording}
          title="取消"
        >
          <X size={16} />
        </button>

        {/* Recording indicator */}
        <div className="hx-voice-indicator">
          <span className="hx-voice-dot" />
          <div className="hx-voice-waveform" aria-hidden="true">
            {Array.from({ length: 12 }).map((_, i) => (
              <span
                key={i}
                className="hx-voice-bar"
                style={{
                  height: `${Math.max(3, volume * 20 * (0.5 + Math.random() * 0.5))}px`,
                  animationDelay: `${i * 0.05}s`,
                }}
              />
            ))}
          </div>
          <span className="hx-voice-duration">{formatDuration(duration)}</span>
        </div>

        {/* Send button */}
        <button
          type="button"
          className="hx-voice-send-btn"
          onClick={stopRecording}
          title="发送"
        >
          <Send size={16} />
        </button>
      </div>
    );
  }

  // Error state — show with mic-off icon, click to retry
  if (state === 'error') {
    return (
      <button
        type="button"
        className="hx-input-tool-btn hx-voice-btn-error"
        onClick={startRecording}
        title={errorMessage || '录音出错，点击重试'}
      >
        <MicOff size={16} />
      </button>
    );
  }

  // Default idle state — mic button in toolbar
  // Always enabled; startRecording handles unsupported environments with error state
  return (
    <button
      type="button"
      className="hx-input-tool-btn"
      onClick={startRecording}
      disabled={disabled}
      title="语音输入"
    >
      <Mic size={16} />
    </button>
  );
}
