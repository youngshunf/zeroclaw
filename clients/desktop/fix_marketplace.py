import re

with open('src/huanxing/pages/Marketplace.tsx', 'r') as f:
    text = f.read()

# Replace InstallModal styles
text = text.replace(
    '''      <div style={{ background: 'var(--hx-bg-panel)' }} className="rounded-2xl w-[460px] max-w-[90vw] shadow-2xl flex flex-col overflow-hidden animate-in fade-in zoom-in duration-200 border border-[var(--hx-border)]">''',
    '''      <div className="bg-hx-bg-panel rounded-2xl w-[460px] max-w-[90vw] shadow-2xl flex flex-col overflow-hidden animate-in fade-in zoom-in duration-200 border border-hx-border">'''
)
text = text.replace(
    '''        <div style={{ borderColor: 'var(--hx-border)' }} className="px-6 py-4 border-b flex items-center gap-3">''',
    '''        <div className="border-hx-border px-6 py-4 border-b flex items-center gap-3">'''
)
text = text.replace(
    '''            <h2 style={{ color: 'var(--hx-text-primary)' }} className="text-base font-bold leading-tight">{titlePrefix}：{targetName}</h2>''',
    '''            <h2 className="text-hx-text-primary text-base font-bold leading-tight">{titlePrefix}：{targetName}</h2>'''
)
text = text.replace(
    '''            <p style={{ color: 'var(--hx-text-secondary)' }} className="text-xs">智能自动化部署</p>''',
    '''            <p className="text-hx-text-secondary text-xs">智能自动化部署</p>'''
)
text = text.replace(
    '''                  <label style={{ color: 'var(--hx-text-primary)' }} className="block text-sm font-medium mb-1">为您的新 Agent 命名</label>''',
    '''                  <label className="text-hx-text-primary block text-sm font-medium mb-1">为您的新 Agent 命名</label>'''
)
text = text.replace(
    '''                    style={{ background: 'var(--hx-bg-input)', borderColor: 'var(--hx-border)', color: 'var(--hx-text-primary)' }}
                    className="w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-indigo-500/50 text-sm"''',
    '''                    className="bg-hx-bg-input border-hx-border text-hx-text-primary w-full px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-indigo-500/50 text-sm"'''
)
text = text.replace(
    '''              <p style={{ background: 'var(--hx-purple-bg)', color: 'var(--hx-text-secondary)', borderColor: 'var(--hx-border)' }} className="text-xs p-2.5 rounded-md border text-center">''',
    '''              <p className="bg-hx-purple-bg text-hx-text-secondary border-hx-border text-xs p-2.5 rounded-md border text-center">'''
)
text = text.replace(
    '''                <span className="text-sm font-medium" style={{ color: 'var(--hx-text-primary)' }}>正在拉取与配置资源...</span>''',
    '''                <span className="text-sm font-medium text-hx-text-primary">正在拉取与配置资源...</span>'''
)
text = text.replace(
    '''              <div 
                ref={scrollRef}
                style={{ background: 'var(--hx-bg-input)', color: 'var(--hx-text-secondary)', border: '1px solid var(--hx-border)' }}
                className="rounded-lg p-3 h-48 overflow-y-auto font-mono text-xs shadow-inner whitespace-pre-wrap"
              >''',
    '''              <div 
                ref={scrollRef}
                className="bg-hx-bg-input text-hx-text-secondary border border-hx-border rounded-lg p-3 h-48 overflow-y-auto font-mono text-xs shadow-inner whitespace-pre-wrap"
              >'''
)

text = text.replace(
    '''                    <div key={idx} style={{ color }} className={`mb-1.5 flex items-start gap-1.5 leading-tight`}>
                      <span style={{ color: 'var(--hx-text-tertiary)' }} className="select-none shrink-0 font-medium">[{idx + 1 < 10 ? `0${idx+1}` : idx+1}]</span>''',
    '''                    <div key={idx} style={{ color }} className={`mb-1.5 flex items-start gap-1.5 leading-tight`}>
                      <span className="text-hx-text-tertiary select-none shrink-0 font-medium">[{idx + 1 < 10 ? `0${idx+1}` : idx+1}]</span>'''
)

text = text.replace(
    '''              <h3 style={{ color: 'var(--hx-text-primary)' }} className="text-lg font-bold mb-1">安装完成！</h3>''',
    '''              <h3 className="text-hx-text-primary text-lg font-bold mb-1">安装完成！</h3>'''
)
text = text.replace(
    '''              <p style={{ color: 'var(--hx-text-secondary)' }} className="text-sm max-w-[80%]">组件已赋能成功，现在可以前往工作台查看与使用。</p>''',
    '''              <p className="text-hx-text-secondary text-sm max-w-[80%]">组件已赋能成功，现在可以前往工作台查看与使用。</p>'''
)
text = text.replace(
    '''              <h3 style={{ color: 'var(--hx-text-primary)' }} className="text-lg font-bold mb-2">安装意外中止</h3>''',
    '''              <h3 className="text-hx-text-primary text-lg font-bold mb-2">安装意外中止</h3>'''
)
text = text.replace(
    '''        <div style={{ background: 'var(--hx-bg-main)', borderColor: 'var(--hx-border)' }} className="px-6 py-4 border-t flex justify-end gap-2">''',
    '''        <div className="bg-hx-bg-main border-hx-border px-6 py-4 border-t flex justify-end gap-2">'''
)
text = text.replace(
    '''              style={{ color: 'var(--hx-text-secondary)' }}
              className="px-4 py-2 text-sm font-medium hover:text-[var(--hx-text-primary)] hover:bg-[var(--hx-bg-input)] rounded-lg transition-colors"''',
    '''              className="text-hx-text-secondary px-4 py-2 text-sm font-medium hover:text-hx-text-primary hover:bg-hx-bg-input rounded-lg transition-colors"'''
)
text = text.replace(
    '''        <div key={app.id} style={{ borderRadius: 'var(--hx-radius-md)', border: '1px solid var(--hx-border)', background: 'var(--hx-bg-panel)', padding: 16, boxShadow: 'var(--hx-shadow-sm)', display: 'flex', flexDirection: 'column' }}>''',
    '''        <div key={app.id} className="rounded-hx-radius-md border border-hx-border bg-hx-bg-panel p-4 shadow-hx-shadow-sm flex flex-col">'''
)
text = text.replace(
    '''                <h3 style={{ fontWeight: 600, color: 'var(--hx-text-primary)', fontSize: 15, lineHeight: 1.3 }}>{app.name}</h3>''',
    '''                <h3 className="font-semibold text-hx-text-primary text-[15px] leading-tight">{app.name}</h3>'''
)
text = text.replace(
    '''                  <span style={{ fontSize: 10, color: 'var(--hx-text-tertiary)' }}>{app.skill_dependencies.split(',').length} 项技能</span>''',
    '''                  <span className="text-[10px] text-hx-text-tertiary">{app.skill_dependencies.split(',').length} 项技能</span>'''
)
text = text.replace(
    '''              <span style={{ fontSize: 11, color: 'var(--hx-text-secondary)', background: 'var(--hx-bg-input)', padding: '4px 10px', borderRadius: 9999 }}>{app.category || 'App'}</span>''',
    '''              <span className="text-[11px] text-hx-text-secondary bg-hx-bg-input px-2.5 py-1 rounded-full">{app.category || 'App'}</span>'''
)
text = text.replace(
    '''          <p style={{ fontSize: 13, color: 'var(--hx-text-secondary)', marginBottom: 16, flex: 1, display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical', overflow: 'hidden', height: 40 }}>{app.description}</p>''',
    '''          <p className="text-[13px] text-hx-text-secondary mb-4 flex-1 line-clamp-2 overflow-hidden h-10">{app.description}</p>'''
)
text = text.replace(
    '''            <span style={{ fontSize: 12, color: 'var(--hx-text-tertiary)' }}>v{app.latest_version || '1.0.0'}</span>''',
    '''            <span className="text-xs text-hx-text-tertiary">v{app.latest_version || '1.0.0'}</span>'''
)

text = text.replace(
    '''          <div key={skill.id} style={{ borderRadius: 'var(--hx-radius-md)', border: '1px solid var(--hx-border)', background: 'var(--hx-bg-panel)', padding: 16, boxShadow: 'var(--hx-shadow-sm)', display: 'flex', flexDirection: 'column' }}>''',
    '''          <div key={skill.id} className="rounded-hx-radius-md border border-hx-border bg-hx-bg-panel p-4 shadow-hx-shadow-sm flex flex-col">'''
)
text = text.replace(
    '''                <h3 style={{ fontWeight: 600, color: 'var(--hx-text-primary)', lineHeight: 1.3 }}>{skill.name}</h3>''',
    '''                <h3 className="font-semibold text-hx-text-primary leading-tight">{skill.name}</h3>'''
)
text = text.replace(
    '''              <span style={{ fontSize: 10, color: 'var(--hx-text-secondary)', background: 'var(--hx-bg-input)', padding: '4px 8px', borderRadius: 9999 }}>{skill.category || 'Skill'}</span>''',
    '''              <span className="text-[10px] text-hx-text-secondary bg-hx-bg-input px-2 py-1 rounded-full">{skill.category || 'Skill'}</span>'''
)
text = text.replace(
    '''            <p style={{ fontSize: 13, color: 'var(--hx-text-secondary)', marginBottom: 16, flex: 1, display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical', overflow: 'hidden' }}>{skill.description}</p>''',
    '''            <p className="text-[13px] text-hx-text-secondary mb-4 flex-1 line-clamp-2 overflow-hidden">{skill.description}</p>'''
)
text = text.replace(
    '''              <span style={{ fontSize: 12, color: 'var(--hx-text-tertiary)' }}>v{skill.latest_version || '1.0.0'}</span>''',
    '''              <span className="text-xs text-hx-text-tertiary">v{skill.latest_version || '1.0.0'}</span>'''
)


text = text.replace(
    '''          <div key={sop.id} style={{ borderRadius: 'var(--hx-radius-md)', border: '1px solid var(--hx-border)', background: 'var(--hx-bg-panel)', padding: 16, boxShadow: 'var(--hx-shadow-sm)', display: 'flex', flexDirection: 'column' }}>''',
    '''          <div key={sop.id} className="rounded-hx-radius-md border border-hx-border bg-hx-bg-panel p-4 shadow-hx-shadow-sm flex flex-col">'''
)
text = text.replace(
    '''                <h3 style={{ fontWeight: 600, color: 'var(--hx-text-primary)', lineHeight: 1.3 }}>{sop.name}</h3>''',
    '''                <h3 className="font-semibold text-hx-text-primary leading-tight">{sop.name}</h3>'''
)
text = text.replace(
    '''                <span style={{ fontSize: 10, color: 'var(--hx-blue)', background: 'var(--hx-purple-bg)', padding: '2px 8px', borderRadius: 9999 }}>{modeLabel(sop.execution_mode)}</span>
                <span style={{ fontSize: 10, color: 'var(--hx-text-secondary)', background: 'var(--hx-bg-input)', padding: '2px 8px', borderRadius: 9999 }}>{sop.category || 'SOP'}</span>''',
    '''                <span className="text-[10px] text-hx-blue bg-hx-purple-bg px-2 py-0.5 rounded-full">{modeLabel(sop.execution_mode)}</span>
                <span className="text-[10px] text-hx-text-secondary bg-hx-bg-input px-2 py-0.5 rounded-full">{sop.category || 'SOP'}</span>'''
)
text = text.replace(
    '''            <p style={{ fontSize: 13, color: 'var(--hx-text-secondary)', marginBottom: 12, flex: 1, display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical', overflow: 'hidden' }}>{sop.description}</p>''',
    '''            <p className="text-[13px] text-hx-text-secondary mb-3 flex-1 line-clamp-2 overflow-hidden">{sop.description}</p>'''
)
text = text.replace(
    '''              <span style={{ fontSize: 12, color: 'var(--hx-text-tertiary)' }}>v{sop.latest_version || '1.0.0'}</span>''',
    '''              <span className="text-xs text-hx-text-tertiary">v{sop.latest_version || '1.0.0'}</span>'''
)

text = text.replace(
    '''    <div style={{ background: 'var(--hx-bg-panel)', padding: 16, borderRadius: 'var(--hx-radius-md)', border: '1px solid var(--hx-border)' }}>
      <label style={{ display: 'block', fontSize: 13, fontWeight: 500, color: 'var(--hx-text-secondary)', marginBottom: 8 }}>{label}</label>''',
    '''    <div className="bg-hx-bg-panel p-4 rounded-hx-radius-md border border-hx-border">
      <label className="block text-[13px] font-medium text-hx-text-secondary mb-2">{label}</label>'''
)
text = text.replace(
    '''        <SelectTrigger style={{ width: '100%', maxWidth: 256, background: 'var(--hx-bg-input)', color: 'var(--hx-text-primary)', borderColor: 'var(--hx-border)' }}>''',
    '''        <SelectTrigger className="w-full max-w-64 bg-hx-bg-input text-hx-text-primary border-hx-border">'''
)
text = text.replace(
    '''              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                {a.icon_url ? (
                  <img src={a.icon_url} alt={a.name} style={{ width: 16, height: 16, borderRadius: 4, objectFit: 'cover' }} />
                ) : (
                  <Bot size={16} />
                )}''',
    '''              <div className="flex items-center gap-2">
                {a.icon_url ? (
                  <img src={a.icon_url} alt={a.name} className="w-4 h-4 rounded object-cover" />
                ) : (
                  <Bot className="w-4 h-4" />
                )}'''
)

text = text.replace(
    '''    <div style={{ display: 'flex', height: '100%', width: '100%', flexDirection: 'column', background: 'var(--hx-bg-main)', minWidth: 0, color: 'var(--hx-text-primary)' }}>''',
    '''    <div className="flex h-full w-full flex-col bg-hx-bg-main min-w-0 text-hx-text-primary">'''
)
text = text.replace(
    '''      <div 
        style={{ flexShrink: 0, borderBottom: '1px solid var(--hx-border)', background: 'var(--hx-bg-panel)', padding: '20px 24px 12px', position: 'relative', zIndex: 10, WebkitAppRegion: 'drag' } as React.CSSProperties}
        data-tauri-drag-region
      >
        <div style={{ WebkitAppRegion: 'no-drag', display: 'flex', alignItems: 'center', justifyContent: 'center', width: '100%' } as React.CSSProperties}>
          <div style={{ display: 'flex', gap: 6, background: 'var(--hx-bg-input)', padding: 6, borderRadius: 'var(--hx-radius-md)' }}>''',
    '''      <div 
        className="shrink-0 border-b border-hx-border bg-hx-bg-panel pt-5 px-6 pb-3 relative z-10"
        style={{ WebkitAppRegion: 'drag' } as React.CSSProperties}
        data-tauri-drag-region
      >
        <div className="flex items-center justify-center w-full" style={{ WebkitAppRegion: 'no-drag' } as React.CSSProperties}>
          <div className="flex gap-1.5 bg-hx-bg-input p-1.5 rounded-hx-radius-md">'''
)

tab_btn_old = '''              <button
                key={t.key}
                onClick={() => setTab(t.key)}
                style={{
                  display: 'flex', alignItems: 'center', padding: '6px 16px', fontSize: 13, fontWeight: 500,
                  borderRadius: 'var(--hx-radius-sm)', transition: 'all 0.15s', border: 'none', cursor: 'pointer',
                  background: tab === t.key ? 'var(--hx-bg-main)' : 'transparent',
                  color: tab === t.key ? 'var(--hx-text-primary)' : 'var(--hx-text-tertiary)',
                  boxShadow: tab === t.key ? 'var(--hx-shadow-sm)' : 'none',
                }}
              >
                <t.icon style={{ width: 16, height: 16, marginRight: 6 }} /> {t.label}
              </button>'''

tab_btn_new = '''              <button
                key={t.key}
                onClick={() => setTab(t.key)}
                className={`flex items-center px-4 py-1.5 text-[13px] font-medium rounded-hx-radius-sm transition-all duration-150 border-none cursor-pointer ${
                  tab === t.key 
                    ? 'bg-hx-bg-main text-hx-text-primary shadow-hx-shadow-sm' 
                    : 'bg-transparent text-hx-text-tertiary hover:text-hx-text-secondary'
                }`}
              >
                <t.icon className="w-4 h-4 mr-1.5" /> {t.label}
              </button>'''
text = text.replace(tab_btn_old, tab_btn_new)

with open('src/huanxing/pages/Marketplace.tsx', 'w') as f:
    f.write(text)

