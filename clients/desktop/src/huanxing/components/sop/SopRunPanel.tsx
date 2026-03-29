import React, { useState, useEffect, useRef } from 'react';
import { wsMultiplexer } from '@/lib/ws';
import { WsMessage } from '@/types/api';
import { Loader2, Send, CheckCircle2, AlertCircle, Bot, Zap } from 'lucide-react';

interface SopRunPanelProps {
  sessionId: string;
  agentName: string;
  sopName: string;
  onClose: () => void;
}

interface RunLog {
  id: string;
  type: 'system' | 'agent' | 'user' | 'tool_call' | 'tool_result' | 'error';
  content: string;
  params?: any;
  timestamp: Date;
}

export function SopRunPanel({ sessionId, agentName, sopName, onClose }: SopRunPanelProps) {
  const [logs, setLogs] = useState<RunLog[]>([]);
  const [inputStr, setInputStr] = useState('');
  const [connected, setConnected] = useState(false);
  const [running, setRunning] = useState(false);
  const logEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    logEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [logs]);

  useEffect(() => {
    let unmounted = false;

    setLogs([{
      id: 'sys-start',
      type: 'system',
      content: `启动独立执行沙盒：${sopName}`,
      timestamp: new Date()
    }]);

    const handleMessage = (msg: WsMessage) => {
      if (msg.session_id !== sessionId) return;

      setLogs(prev => {
        const ts = new Date();
        const nextId = `${msg.type}-${Date.now()}-${Math.random()}`;
        
        if (msg.type === 'connected' || msg.type === 'session_start') {
          setConnected(true);
          return prev;
        }

        if (msg.type === 'chunk' || msg.type === 'done') {
          const last = prev[prev.length - 1];
          let content = msg.type === 'done' ? (msg.full_response || '') : (msg.content || '');
          if (last && last.type === 'agent' && last.id.startsWith('agent-chunk')) {
            const arr = [...prev];
            arr[arr.length - 1] = { ...last, content: last.content + content };
            if (msg.type === 'done') setRunning(false);
            return arr;
          } else {
            return [...prev, { id: `agent-chunk-${Date.now()}`, type: 'agent', content, timestamp: ts }];
          }
        }

        if (msg.type === 'tool_call') {
          return [...prev, {
            id: nextId,
            type: 'tool_call',
            content: `调用能力: ${msg.display_name || msg.name}`,
            params: { args: msg.args_preview },
            timestamp: ts
          }];
        }

        if (msg.type === 'tool_result') {
          return [...prev, {
            id: nextId,
            type: 'tool_result',
            content: `结果: ${msg.status === 'success' ? '成功' : '失败'} (${msg.duration_ms}ms)`,
            timestamp: ts
          }];
        }

        if (msg.type === 'error') {
          setRunning(false);
          return [...prev, { id: nextId, type: 'error', content: msg.message || 'Unknown error', timestamp: ts }];
        }

        return prev;
      });
    };

    const unsubscribe = wsMultiplexer.subscribe(sessionId, handleMessage);

    setTimeout(() => {
      if (!unmounted) {
        setRunning(true);
        wsMultiplexer.send(sessionId, `请立即开始执行工作流：${sopName}`, agentName);
      }
    }, 500);

    return () => {
      unmounted = true;
      unsubscribe();
    };
  }, [sessionId, agentName, sopName]);

  const handleSend = () => {
    if (!inputStr.trim()) return;
    setRunning(true);
    setLogs(prev => [...prev, {
      id: `user-${Date.now()}`,
      type: 'user',
      content: inputStr,
      timestamp: new Date()
    }]);

    wsMultiplexer.send(sessionId, inputStr, agentName);
    setInputStr('');
  };

  const getLogStyle = (type: RunLog['type']): React.CSSProperties => {
    const base: React.CSSProperties = {
      padding: 10, borderRadius: 'var(--hx-radius-sm)', display: 'flex', gap: 12,
      border: '1px solid var(--hx-border)', fontSize: 12, fontFamily: 'monospace',
    };
    switch (type) {
      case 'system':
        return { ...base, background: 'var(--hx-purple-bg)', color: 'var(--hx-purple)', borderColor: 'var(--hx-border)' };
      case 'tool_call':
        return { ...base, background: 'var(--hx-purple-bg)', color: 'var(--hx-blue)', borderColor: 'var(--hx-border)' };
      case 'tool_result':
        return { ...base, background: 'rgba(16,185,129,0.06)', color: 'var(--hx-green)', borderColor: 'rgba(16,185,129,0.2)' };
      case 'error':
        return { ...base, background: 'rgba(239,68,68,0.06)', color: 'var(--hx-red)', borderColor: 'rgba(239,68,68,0.2)' };
      case 'user':
        return { ...base, background: 'var(--hx-bg-panel)', color: 'var(--hx-text-primary)', borderColor: 'var(--hx-border)', marginLeft: 48, textAlign: 'right' };
      case 'agent':
        return { ...base, background: 'var(--hx-bg-panel)', color: 'var(--hx-text-primary)', borderColor: 'var(--hx-border)', marginRight: 48, boxShadow: 'var(--hx-shadow-sm)' };
      default:
        return base;
    }
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: 'var(--hx-bg-main)', border: '1px solid var(--hx-border)', borderRadius: 'var(--hx-radius-md)', overflow: 'hidden', boxShadow: 'var(--hx-shadow-md)' }}>
      {/* Header */}
      <div style={{ background: 'var(--hx-bg-panel)', borderBottom: '1px solid var(--hx-border)', padding: '12px 16px', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, color: 'var(--hx-text-primary)' }}>
          <Zap style={{ width: 16, height: 16, color: 'var(--hx-amber)' }} />
          <span style={{ fontWeight: 600, fontSize: 14 }}>{sopName} (运行中)</span>
          {running && <Loader2 style={{ width: 14, height: 14, color: 'var(--hx-purple)', animation: 'hx-spin 1s linear infinite' }} />}
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <span style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 12, color: connected ? 'var(--hx-green)' : 'var(--hx-text-tertiary)' }}>
            <span style={{ width: 8, height: 8, borderRadius: '50%', background: connected ? 'var(--hx-green)' : 'var(--hx-text-tertiary)' }} />
            {connected ? '已连接' : '连接中...'}
          </span>
          <button onClick={onClose} style={{ color: 'var(--hx-text-secondary)', fontSize: 13, padding: '4px 8px', background: 'var(--hx-bg-input)', borderRadius: 'var(--hx-radius-sm)', border: '1px solid var(--hx-border)', cursor: 'pointer' }}>
            关闭 / 隐藏
          </button>
        </div>
      </div>

      {/* Logs Area */}
      <div style={{ flex: 1, overflowY: 'auto', padding: 16, display: 'flex', flexDirection: 'column', gap: 12 }}>
        {logs.map(log => (
          <div key={log.id} style={getLogStyle(log.type)}>
            {log.type === 'tool_result' && <CheckCircle2 style={{ width: 14, height: 14, marginTop: 2, flexShrink: 0 }} />}
            {log.type === 'error' && <AlertCircle style={{ width: 14, height: 14, marginTop: 2, flexShrink: 0 }} />}
            {log.type === 'agent' && <Bot style={{ width: 16, height: 16, marginTop: 2, color: 'var(--hx-purple)', flexShrink: 0 }} />}
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 4 }}>
                {log.type === 'user' && <span style={{ fontSize: 10, background: 'var(--hx-bg-input)', padding: '2px 6px', borderRadius: 4, color: 'var(--hx-text-tertiary)', marginRight: 8 }}>人工指引</span>}
                <span style={{ fontSize: 10, color: 'var(--hx-text-tertiary)' }}>{log.timestamp.toLocaleTimeString()}</span>
              </div>
              <div style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>{log.content}</div>
              {log.params?.args && (
                <pre style={{ marginTop: 8, fontSize: 10, background: 'var(--hx-bg-input)', padding: 8, borderRadius: 'var(--hx-radius-sm)', overflowX: 'auto', color: 'var(--hx-text-secondary)' }}>
                  {log.params.args}
                </pre>
              )}
            </div>
          </div>
        ))}
        <div ref={logEndRef} />
      </div>

      {/* Input Area */}
      <div style={{ background: 'var(--hx-bg-panel)', borderTop: '1px solid var(--hx-border)', padding: 12, display: 'flex', gap: 8 }}>
        <input
          type="text"
          value={inputStr}
          onChange={e => setInputStr(e.target.value)}
          onKeyDown={e => { if (e.key === 'Enter') handleSend(); }}
          placeholder="提供审批意见或干预执行流..."
          style={{
            flex: 1, background: 'var(--hx-bg-input)', border: '1px solid var(--hx-border)',
            borderRadius: 'var(--hx-radius-sm)', padding: '8px 12px', fontSize: 13,
            color: 'var(--hx-text-primary)', outline: 'none',
          }}
        />
        <button
          onClick={handleSend}
          disabled={!inputStr.trim()}
          style={{
            background: 'var(--hx-purple)', color: '#fff', padding: '8px 16px',
            borderRadius: 'var(--hx-radius-sm)', border: 'none', cursor: 'pointer',
            opacity: inputStr.trim() ? 1 : 0.5, display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}
        >
          <Send style={{ width: 16, height: 16 }} />
        </button>
      </div>
    </div>
  );
}
