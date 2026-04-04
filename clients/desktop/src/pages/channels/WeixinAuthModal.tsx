import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AlertCircle, CheckCircle2, Bot } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/Dialog';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/Select';
import { listAgents, type AgentInfo } from '@/lib/agent-api';
import { resolveApiUrl } from '@/config';

export default function WeixinAuthModal({ open, onOpenChange }: { open: boolean; onOpenChange: (v: boolean) => void }) {
  const [qrState, setQrState] = useState<'loading' | 'waiting' | 'confirmed' | 'error'>('loading');
  const [qrcodeUrl, setQrcodeUrl] = useState<string>('');
  const [sessionKey, setSessionKey] = useState<string>('');
  const [qrcodeId, setQrcodeId] = useState<string>('');
  const [botId, setBotId] = useState<string>('');
  const [botToken, setBotToken] = useState<string>('');
  const [errorMsg, setErrorMsg] = useState<string>('');

  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [selectedAgent, setSelectedAgent] = useState<string>('');
  const [isBinding, setIsBinding] = useState(false);

  // 1. 获取二维码
  useEffect(() => {
    if (!open) return;
    let canceled = false;
    setQrState('loading');
    setErrorMsg('');
    async function initQr() {
      try {
        const res: any = await invoke('generate_weixin_qr');
        if (canceled) return;
        setQrcodeUrl(res.qrcode_url);
        setSessionKey(res.session_key);
        setQrcodeId(res.qrcode_id);
        setQrState('waiting');
      } catch (err: any) {
        if (!canceled) {
          setErrorMsg(err.toString());
          setQrState('error');
        }
      }
    }
    initQr();
    return () => { canceled = true; };
  }, [open]);

  // 2. 长轮询
  useEffect(() => {
    if (qrState !== 'waiting' || !qrcodeId || !sessionKey) return;
    let timer: ReturnType<typeof setTimeout>;
    let canceled = false;

    const poll = async () => {
      if (canceled) return;
      try {
        const res: any = await invoke('poll_weixin_auth_status', {
          sessionKey,
          qrcode: qrcodeId,
        });
        if (res.status === 'confirmed') {
          setBotId(res.ilink_bot_id || '');
          setBotToken(res.bot_token || '');
          setQrState('confirmed');
          loadAgents();
          return;
        } else if (res.status === 'expired') {
          setErrorMsg('二维码已过期，请重新打开。');
          setQrState('error');
          return;
        }
      } catch (e: any) {
        console.error('Poll error:', e);
      }
      timer = setTimeout(poll, 3000);
    };
    poll();
    return () => { canceled = true; clearTimeout(timer); };
  }, [qrState, qrcodeId, sessionKey]);

  // 3. 获取本地代理 — 使用公共 agent-api
  const loadAgents = async () => {
    try {
      const data = await listAgents();
      const list = data.agents || [];
      setAgents(list);
      if (list.length > 0) setSelectedAgent(list[0].name);
    } catch (e: any) {
      console.error(e);
      setErrorMsg('获取助手列表失败: ' + e);
    }
  };

  // 4. 提交绑定
  const handleBind = async () => {
    if (!selectedAgent || !botId) return;
    setIsBinding(true);
    try {
      await invoke('save_weixin_credentials', { botToken, botId, baseUrl: null });
      await invoke('bind_channel_to_agent', { channelType: 'weixin', senderId: botId, agentId: selectedAgent });
      onOpenChange(false);
    } catch (e: any) {
      setErrorMsg('绑定失败: ' + e);
      setIsBinding(false);
    }
  };

  // Normalize QR code URL
  const normalizedQrUrl = (() => {
    if (!qrcodeUrl) return '';
    if (qrcodeUrl.startsWith('data:')) return qrcodeUrl;
    if (qrcodeUrl.startsWith('http')) return qrcodeUrl;
    return `data:image/png;base64,${qrcodeUrl}`;
  })();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-sm">
        {/* Loading */}
        {qrState === 'loading' && (
          <>
            <DialogHeader>
              <DialogTitle>微信扫码授权</DialogTitle>
              <DialogDescription>正在获取安全二维码...</DialogDescription>
            </DialogHeader>
            <div className="flex items-center justify-center py-12">
              <div className="hx-spinner" />
            </div>
          </>
        )}

        {/* QR Code */}
        {qrState === 'waiting' && normalizedQrUrl && (
          <>
            <DialogHeader>
              <DialogTitle>请使用微信扫码授权</DialogTitle>
              <DialogDescription>扫码后请在手机端点击确认，授权通过后当前设备将充当消息接收端。</DialogDescription>
            </DialogHeader>
            <div className="flex justify-center py-4">
              <div className="bg-white p-2 rounded-xl shadow-sm">
                <img
                  src={normalizedQrUrl}
                  alt="微信二维码"
                  className="w-48 h-48 object-contain block"
                />
              </div>
            </div>
          </>
        )}

        {/* Confirmed — 使用公共 AgentSelector (Radix Select) */}
        {qrState === 'confirmed' && (
          <>
            <DialogHeader>
              <div className="flex items-center gap-3 mb-1">
                <div className="w-10 h-10 rounded-full bg-emerald-500/15 flex items-center justify-center shrink-0">
                  <CheckCircle2 className="w-6 h-6 text-emerald-500" />
                </div>
                <div>
                  <DialogTitle>授权成功</DialogTitle>
                  <DialogDescription>微信账号（{botId}）已接入，请选择一个 AI 助手来接管。</DialogDescription>
                </div>
              </div>
            </DialogHeader>

            <div className="space-y-3 pt-2">
              <div>
                <label className="block text-xs font-medium text-hx-text-tertiary tracking-wide mb-1.5">
                  分配接管助手
                </label>
                <Select value={selectedAgent} onValueChange={setSelectedAgent} disabled={isBinding}>
                  <SelectTrigger className="w-full bg-hx-bg-input text-hx-text-primary border-hx-border">
                    <SelectValue placeholder="选择目标 Agent" />
                  </SelectTrigger>
                  <SelectContent>
                    {agents.map(a => (
                      <SelectItem key={a.name} value={a.name}>
                        <div className="flex items-center gap-2">
                          {a.icon_url ? (
                            <img src={resolveApiUrl(a.icon_url)} alt={a.name} className="w-4 h-4 rounded object-cover" />
                          ) : (
                            <Bot className="w-4 h-4" />
                          )}
                          <span>{a.display_name || a.name}</span>
                        </div>
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                {agents.length === 0 && (
                  <p className="text-xs text-hx-text-tertiary mt-2">暂无助手，请先前往管理页面创建一个。</p>
                )}
              </div>
            </div>

            <DialogFooter>
              <button
                onClick={() => onOpenChange(false)}
                className="px-4 py-2 text-sm font-medium text-hx-text-secondary hover:text-hx-text-primary hover:bg-hx-bg-input rounded-lg transition-colors"
              >
                取消
              </button>
              <button
                onClick={handleBind}
                disabled={isBinding || !selectedAgent}
                className="px-5 py-2 text-sm font-medium bg-hx-purple hover:bg-hx-purple-hover text-white rounded-lg shadow-sm transition-colors disabled:opacity-50"
              >
                {isBinding ? '正在配置...' : '确认分配并重启引擎'}
              </button>
            </DialogFooter>
          </>
        )}

        {/* Error */}
        {qrState === 'error' && (
          <>
            <DialogHeader>
              <DialogTitle>获取失败</DialogTitle>
              <DialogDescription>请检查网络环境后重试。</DialogDescription>
            </DialogHeader>
            <div className="flex items-center justify-center py-8">
              <AlertCircle className="w-10 h-10 text-red-400" />
            </div>
          </>
        )}

        {/* Error message bar */}
        {errorMsg && (
          <div className="flex items-start gap-2 p-3 rounded-lg bg-red-500/10 border border-red-500/20 text-red-400 text-xs">
            <AlertCircle className="w-3.5 h-3.5 shrink-0 mt-0.5" />
            <span>{errorMsg}</span>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
