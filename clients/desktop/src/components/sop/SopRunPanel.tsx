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

  const getLogStyle = (type: RunLog['type']): string => {
    const base = "p-2.5 rounded-hx-radius-sm flex gap-3 border border-hx-border text-xs font-mono";
    switch (type) {
      case 'system':
        return `${base} bg-hx-purple-bg text-hx-purple`;
      case 'tool_call':
        return `${base} bg-hx-purple-bg text-hx-blue`;
      case 'tool_result':
        return `${base} bg-emerald-500/5 text-hx-green border-emerald-500/20`;
      case 'error':
        return `${base} bg-red-500/5 text-hx-red border-red-500/20`;
      case 'user':
        return `${base} bg-hx-bg-panel text-hx-text-primary ml-12 text-right`;
      case 'agent':
        return `${base} bg-hx-bg-panel text-hx-text-primary mr-12 shadow-hx-shadow-sm`;
      default:
        return base;
    }
  };

  return (
    <div className="flex flex-col h-full bg-hx-bg-main border border-hx-border rounded-hx-radius-md overflow-hidden shadow-hx-shadow-md">
      {/* Header */}
      <div className="bg-hx-bg-panel border-b border-hx-border px-4 py-3 flex items-center justify-between">
        <div className="flex items-center gap-2 text-hx-text-primary">
          <Zap className="w-4 h-4 text-hx-amber" />
          <span className="font-semibold text-sm">{sopName} (运行中)</span>
          {running && <Loader2 className="w-3.5 h-3.5 text-hx-purple animate-spin" />}
        </div>
        <div className="flex items-center gap-3">
          <span className={`flex items-center gap-1.5 text-xs ${connected ? 'text-hx-green' : 'text-hx-text-tertiary'}`}>
            <span className={`w-2 h-2 rounded-full ${connected ? 'bg-hx-green' : 'bg-hx-text-tertiary'}`} />
            {connected ? '已连接' : '连接中...'}
          </span>
          <button onClick={onClose} className="text-hx-text-secondary text-[13px] px-2 py-1 bg-hx-bg-input rounded-hx-radius-sm border border-hx-border cursor-pointer">
            关闭 / 隐藏
          </button>
        </div>
      </div>

      {/* Logs Area */}
      <div className="flex-1 overflow-y-auto p-4 flex flex-col gap-3">
        {logs.map(log => (
          <div key={log.id} className={getLogStyle(log.type)}>
            {log.type === 'tool_result' && <CheckCircle2 className="w-3.5 h-3.5 mt-0.5 shrink-0" />}
            {log.type === 'error' && <AlertCircle className="w-3.5 h-3.5 mt-0.5 shrink-0" />}
            {log.type === 'agent' && <Bot className="w-4 h-4 mt-0.5 text-hx-purple shrink-0" />}
            <div className="flex-1 min-w-0">
              <div className="flex items-center justify-between mb-1">
                {log.type === 'user' && <span className="text-[10px] bg-hx-bg-input px-1.5 py-0.5 rounded text-hx-text-tertiary mr-2">人工指引</span>}
                <span className="text-[10px] text-hx-text-tertiary">{log.timestamp.toLocaleTimeString()}</span>
              </div>
              <div className="whitespace-pre-wrap break-words">{log.content}</div>
              {log.params?.args && (
                <pre className="mt-2 text-[10px] bg-hx-bg-input p-2 rounded-hx-radius-sm overflow-x-auto text-hx-text-secondary">
                  {log.params.args}
                </pre>
              )}
            </div>
          </div>
        ))}
        <div ref={logEndRef} />
      </div>

      {/* Input Area */}
      <div className="bg-hx-bg-panel border-t border-hx-border p-3 flex gap-2">
        <input
          type="text"
          value={inputStr}
          onChange={e => setInputStr(e.target.value)}
          onKeyDown={e => { if (e.key === 'Enter') handleSend(); }}
          placeholder="提供审批意见或干预执行流..."
          className="flex-1 bg-hx-bg-input border border-hx-border rounded-hx-radius-sm px-3 py-2 text-[13px] text-hx-text-primary outline-none focus:border-hx-purple"
        />
        <button
          onClick={handleSend}
          disabled={!inputStr.trim()}
          className="bg-hx-purple text-white px-4 py-2 rounded-hx-radius-sm border-none cursor-pointer flex items-center justify-center disabled:opacity-50"
        >
          <Send className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
}
