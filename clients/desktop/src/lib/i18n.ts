import { useState, useEffect } from 'react';
import { getStatus } from './api';

// ---------------------------------------------------------------------------
// Translation dictionaries
// ---------------------------------------------------------------------------

export type Locale = 'en' | 'tr' | 'zh-CN' | 'ja' | 'ru' | 'fr' | 'vi' | 'el';

export const LANGUAGE_SWITCH_ORDER: ReadonlyArray<Locale> = [
  'en',
  'zh-CN',
  'ja',
  'ru',
  'fr',
  'vi',
  'el',
];

export const LANGUAGE_BUTTON_LABELS: Record<Locale, string> = {
  en: 'EN',
  tr: 'TR',
  'zh-CN': '简体',
  ja: '日本語',
  ru: 'РУ',
  fr: 'FR',
  vi: 'VI',
  el: 'ΕΛ',
};

const KNOWN_LOCALES: ReadonlyArray<Locale> = [
  'en',
  'tr',
  'zh-CN',
  'ja',
  'ru',
  'fr',
  'vi',
  'el',
];

const translations: Record<Locale, Record<string, string>> = {
  en: {
    // Navigation
    'nav.dashboard': 'Dashboard',
    'nav.agent': 'Agent',
    'nav.tools': 'Tools',
    'nav.cron': 'Scheduled Jobs',
    'nav.integrations': 'Integrations',
    'nav.memory': 'Memory',
    'nav.devices': 'Devices',
    'nav.config': 'Configuration',
    'nav.cost': 'Cost Tracker',
    'nav.logs': 'Logs',
    'nav.doctor': 'Doctor',

    // Dashboard
    'dashboard.title': 'Dashboard',
    'dashboard.provider': 'Provider',
    'dashboard.model': 'Model',
    'dashboard.uptime': 'Uptime',
    'dashboard.temperature': 'Temperature',
    'dashboard.gateway_port': 'Gateway Port',
    'dashboard.locale': 'Locale',
    'dashboard.memory_backend': 'Memory Backend',
    'dashboard.paired': 'Paired',
    'dashboard.channels': 'Channels',
    'dashboard.health': 'Health',
    'dashboard.status': 'Status',
    'dashboard.overview': 'Overview',
    'dashboard.system_info': 'System Information',
    'dashboard.quick_actions': 'Quick Actions',

    // Agent / Chat
    'agent.title': 'Agent Chat',
    'agent.send': 'Send',
    'agent.placeholder': 'Type a message...',
    'agent.connecting': 'Connecting...',
    'agent.connected': 'Connected',
    'agent.disconnected': 'Disconnected',
    'agent.reconnecting': 'Reconnecting...',
    'agent.thinking': 'Thinking...',
    'agent.tool_call': 'Tool Call',
    'agent.tool_result': 'Tool Result',

    // Tools
    'tools.title': 'Available Tools',
    'tools.name': 'Name',
    'tools.description': 'Description',
    'tools.parameters': 'Parameters',
    'tools.search': 'Search tools...',
    'tools.empty': 'No tools available.',
    'tools.count': 'Total tools',

    // Cron
    'cron.title': 'Scheduled Jobs',
    'cron.add': 'Add Job',
    'cron.delete': 'Delete',
    'cron.enable': 'Enable',
    'cron.disable': 'Disable',
    'cron.name': 'Name',
    'cron.command': 'Command',
    'cron.schedule': 'Schedule',
    'cron.next_run': 'Next Run',
    'cron.last_run': 'Last Run',
    'cron.last_status': 'Last Status',
    'cron.enabled': 'Enabled',
    'cron.empty': 'No scheduled jobs.',
    'cron.confirm_delete': 'Are you sure you want to delete this job?',

    // Integrations
    'integrations.title': 'Integrations',
    'integrations.available': 'Available',
    'integrations.active': 'Active',
    'integrations.coming_soon': 'Coming Soon',
    'integrations.category': 'Category',
    'integrations.status': 'Status',
    'integrations.search': 'Search integrations...',
    'integrations.empty': 'No integrations found.',
    'integrations.activate': 'Activate',
    'integrations.deactivate': 'Deactivate',

    // Memory
    'memory.title': 'Memory Store',
    'memory.search': 'Search memory...',
    'memory.add': 'Store Memory',
    'memory.delete': 'Delete',
    'memory.key': 'Key',
    'memory.content': 'Content',
    'memory.category': 'Category',
    'memory.timestamp': 'Timestamp',
    'memory.session': 'Session',
    'memory.score': 'Score',
    'memory.empty': 'No memory entries found.',
    'memory.confirm_delete': 'Are you sure you want to delete this memory entry?',
    'memory.all_categories': 'All Categories',

    // Config
    'config.title': 'Configuration',
    'config.save': 'Save',
    'config.reset': 'Reset',
    'config.saved': 'Configuration saved successfully.',
    'config.error': 'Failed to save configuration.',
    'config.loading': 'Loading configuration...',
    'config.editor_placeholder': 'TOML configuration...',

    // Cost
    'cost.title': 'Cost Tracker',
    'cost.session': 'Session Cost',
    'cost.daily': 'Daily Cost',
    'cost.monthly': 'Monthly Cost',
    'cost.total_tokens': 'Total Tokens',
    'cost.request_count': 'Requests',
    'cost.by_model': 'Cost by Model',
    'cost.model': 'Model',
    'cost.tokens': 'Tokens',
    'cost.requests': 'Requests',
    'cost.usd': 'Cost (USD)',

    // Logs
    'logs.title': 'Live Logs',
    'logs.clear': 'Clear',
    'logs.pause': 'Pause',
    'logs.resume': 'Resume',
    'logs.filter': 'Filter logs...',
    'logs.empty': 'No log entries.',
    'logs.connected': 'Connected to event stream.',
    'logs.disconnected': 'Disconnected from event stream.',

    // Doctor
    'doctor.title': 'System Diagnostics',
    'doctor.run': 'Run Diagnostics',
    'doctor.running': 'Running diagnostics...',
    'doctor.ok': 'OK',
    'doctor.warn': 'Warning',
    'doctor.error': 'Error',
    'doctor.severity': 'Severity',
    'doctor.category': 'Category',
    'doctor.message': 'Message',
    'doctor.empty': 'No diagnostics have been run yet.',
    'doctor.summary': 'Diagnostic Summary',

    // Auth / Pairing
    'auth.pair': 'Pair Device',
    'auth.pairing_code': 'Pairing Code',
    'auth.pair_button': 'Pair',
    'auth.logout': 'Logout',
    'auth.pairing_success': 'Pairing successful!',
    'auth.pairing_failed': 'Pairing failed. Please try again.',
    'auth.enter_code': 'Enter your pairing code to connect to the agent.',

    // Common
    'common.loading': 'Loading...',
    'common.error': 'An error occurred.',
    'common.retry': 'Retry',
    'common.cancel': 'Cancel',
    'common.confirm': 'Confirm',
    'common.save': 'Save',
    'common.delete': 'Delete',
    'common.edit': 'Edit',
    'common.close': 'Close',
    'common.yes': 'Yes',
    'common.no': 'No',
    'common.search': 'Search...',
    'common.no_data': 'No data available.',
    'common.refresh': 'Refresh',
    'common.back': 'Back',
    'common.actions': 'Actions',
    'common.name': 'Name',
    'common.description': 'Description',
    'common.status': 'Status',
    'common.created': 'Created',
    'common.updated': 'Updated',

    // Health
    'health.title': 'System Health',
    'health.component': 'Component',
    'health.status': 'Status',
    'health.last_ok': 'Last OK',
    'health.last_error': 'Last Error',
    'health.restart_count': 'Restarts',
    'health.pid': 'Process ID',
    'health.uptime': 'Uptime',
    'health.updated_at': 'Last Updated',

    // Settings Panel
    'settings.title': 'Settings',
    'settings.general': 'General',
    'settings.system': 'System',
    'settings.about': 'About',
    'settings.engine': 'AI Engine',
    'settings.about_app': 'About HuanXing',

    // Dashboard Extra
    'dash.load_fail': 'Failed to load dashboard',
    'dash.fail_title': 'Load Failed',
    'dash.subtitle': 'HuanXing Operations Console',
    'dash.desc': 'Real-time status, cost tracking, and channel connectivity at a glance',
    'dash.running': 'Running',
    'dash.unpaired': 'Unpaired',
    'dash.unknown': 'Unknown',
    'dash.since_restart': 'Since last restart',
    'dash.device_paired': 'Device paired',
    'dash.no_paired_device': 'No paired device',
    'dash.cost_title': 'Cost Tracking',
    'dash.cost_subtitle': 'Session, daily, and monthly operational costs',
    'dash.session': 'Current Session',
    'dash.today': 'Today',
    'dash.this_month': 'This Month',
    'dash.total_tokens': 'Total Tokens',
    'dash.request_count': 'Requests',
    'dash.channels_title': 'Channel Status',
    'dash.channels_subtitle': 'Connected channels and status',
    'dash.no_channels': 'No channels connected',
    'dash.connected': 'Connected',
    'dash.disconnected': 'Disconnected',
    'dash.health_title': 'Component Health',
    'dash.health_subtitle': 'Runtime heartbeat and component status',
    'dash.no_health_data': 'No component health data',

    // Integrations
    'integ.active': 'Active',
    'integ.available': 'Available',
    'integ.coming_soon': 'Coming Soon',
    'integ.no_integrations': 'No integrations found.',
    'integ.current_model': 'Current model',
    'integ.custom_model_hint': 'For custom model IDs, use Edit Keys.',
    'integ.default_configured': 'Default provider configured',
    'integ.provider_configured': 'Provider configured',
    'integ.creds_configured': 'Credentials configured',
    'integ.creds_not_configured': 'Credentials not configured',
    'integ.edit_keys': 'Edit Keys',
    'integ.configure': 'Configure',
    'integ.configure_title': 'Configure {name}',
    'integ.enter_update': 'Enter only fields you want to update.',
    'integ.enter_required': 'Enter required fields to configure this integration.',
    'integ.saving_updates_default': 'Saving here updates credentials and switches your default AI provider to',
    'integ.adv_settings': 'For advanced provider settings, use',
    'integ.configuration': 'Configuration',
    'integ.configured_badge': 'Configured',
    'integ.select_recommended': 'Select a recommended model',
    'integ.custom_model': 'Custom model...',
    'integ.clear_current': 'Clear current model',
    'integ.current_value': 'Current value:',
    'integ.replace_current': 'Enter a new value to replace current',
    'integ.enter_value': 'Enter value',
    'integ.leave_empty': 'Type new value, or leave empty to keep current',
    'integ.optional': 'Optional',
    'integ.cancel': 'Cancel',
    'integ.save_config': 'Save Configuration',
    'integ.saving': 'Saving...',
    'integ.apply': 'Apply',

    // Memory Extra
    'mem.load_fail': 'Load Failed',
    'mem.required': 'Key and content are required',
    'mem.save_fail': 'Failed to save memory',
    'mem.del_fail': 'Failed to delete memory',
    'mem.title': 'Memory Management',
    'mem.add': 'Add Memory',
    'mem.search_pl': 'Search memory...',
    'mem.all_cat': 'All Categories',
    'mem.search': 'Search',
    'mem.key_pl': 'e.g. user_preferences',
    'mem.content': 'Content',
    'mem.content_pl': 'Memory content...',
    'mem.cat_opt': 'Category (Optional)',
    'mem.cat_pl': 'e.g. preferences, context',
    'mem.cancel': 'Cancel',
    'mem.saving': 'Saving...',
    'mem.save': 'Save',
    'mem.no_entries': 'No memory entries found',
    'mem.th_key': 'Key',
    'mem.th_content': 'Content',
    'mem.th_cat': 'Category',
    'mem.th_time': 'Time',
    'mem.th_actions': 'Actions',
    'mem.del_confirm': 'Delete?',
    'mem.yes': 'Yes',
    'mem.no': 'No',

    // Config Extra
    'config.editor_title': 'Configuration Editor',
    'config.form': 'Form',
    'config.raw': 'Raw',
    'config.saving': 'Saving...',
    'config.hidden_title': 'Sensitive fields hidden',
    'config.hidden_desc_form': 'Hidden fields show as "Configured (hidden)". Leave unchanged to keep original value, enter new value to update.',
    'config.hidden_desc_raw': 'API keys, tokens, and passwords are hidden. To update, replace the entire hidden wrapper with the new value.',
    'config.select_placeholder': 'Select...',

    // Engine Extra
    'engine.title': 'AI Engine',
    'engine.refresh': 'Refresh',
    'engine.running': 'Running',
    'engine.stopped': 'Stopped',
    'engine.starting': 'Starting...',
    'engine.start': 'Start',
    'engine.restarting': 'Restarting...',
    'engine.restart': 'Restart',
    'engine.stopping': 'Stopping...',
    'engine.stop': 'Stop',
    'engine.model': 'Model',
    'engine.uptime': 'Uptime',
    'engine.memory_backend': 'Memory Backend',
    'engine.restarted_count': 'Auto-restarted {count} times',
    'engine.quick_config': 'Quick Config',
    'engine.loading_config': 'Loading config...',
    'engine.config_helper': 'Config can only be modified in Tauri desktop app. Edit config.toml manually in development mode.',
    'engine.logs_title': 'Execution Logs',
    'engine.lines': '{count} lines',
    'engine.auto_scroll': 'Auto-scroll',
    'engine.clear_logs': 'Clear Logs',
    'engine.export_logs': 'Export Logs',
    'engine.waiting_logs': 'Waiting for logs...',
    'engine.not_running': 'Engine not running',
    'engine.default_model': 'Default Model',
    'engine.not_set': '(Not set)',
    'engine.temperature': 'Temperature',
    'engine.precise': 'Precise 0',
    'engine.balanced': 'Balanced 1',
    'engine.creative': 'Creative 2',
    'engine.autonomy': 'Autonomy Level',
    'engine.supervised': 'Supervised - High risk actions need confirmation',
    'engine.semi': 'Semi-autonomous - Only file deletion needs confirmation',
    'engine.full': 'Fully autonomous - All actions execute automatically',
    'engine.reset': 'Reset',
    'engine.save_restart': 'Save & Restart',

    // Tools Extra
    'tools.load_failed': 'Load Failed',
    'tools.search_pl': 'Search tools...',
    'tools.agent_tools': 'Agent Tools',
    'tools.no_match': 'No matching tools found',
    'tools.schema': 'PARAMETERS SCHEMA',
    'tools.cli_tools': 'CLI Tools',
    'tools.th_name': 'Name',
    'tools.th_path': 'Path',
    'tools.th_version': 'Version',
    'tools.th_category': 'Category',

    // Cron Extra
    'cron_extra.required': 'Schedule expression and command are required',
    'cron_extra.add_failed': 'Failed to add job',
    'cron_extra.del_failed': 'Failed to delete job',
    'cron_extra.title': 'Cron Jobs',
    'cron_extra.add_btn': 'Add Job',
    'cron_extra.add_title': 'Add Cron Job',
    'cron_extra.name_opt': 'Name (Optional)',
    'cron_extra.name_pl': 'e.g., Daily Cleanup',
    'cron_extra.schedule': 'Schedule Expression',
    'cron_extra.schedule_pl': 'e.g., 0 0 * * * (cron spec)',
    'cron_extra.command': 'Command',
    'cron_extra.command_pl': 'e.g., cleanup --older-than 7d',
    'cron_extra.cancel': 'Cancel',
    'cron_extra.adding': 'Adding...',
    'cron_extra.add': 'Add',
    'cron_extra.no_jobs': 'No cron jobs',
    'cron_extra.th_id': 'ID',
    'cron_extra.th_name': 'Name',
    'cron_extra.th_cmd': 'Command',
    'cron_extra.th_next': 'Next Run',
    'cron_extra.th_status': 'Status',
    'cron_extra.th_enabled': 'Enabled',
    'cron_extra.th_actions': 'Actions',
    'cron_extra.enabled': 'Enabled',
    'cron_extra.disabled': 'Disabled',
    'cron_extra.del_confirm': 'Delete?',
    'cron_extra.yes': 'Yes',
    'cron_extra.no': 'No',

    // Cost Extra
    'cost_extra.session': 'Session Cost',
    'cost_extra.daily': 'Daily Cost',
    'cost_extra.monthly': 'Monthly Cost',
    'cost_extra.requests': 'Total Requests',
    'cost_extra.token_stats': 'Token Statistics',
    'cost_extra.total_tokens': 'Total Tokens',
    'cost_extra.avg_tokens': 'Avg Tokens/Req',
    'cost_extra.cost_per_1k': 'Cost per 1k Tokens',
    'cost_extra.model_breakdown': 'Model Breakdown',
    'cost_extra.no_data': 'No model data available',
    'cost_extra.th_model': 'Model',
    'cost_extra.th_cost': 'Cost',
    'cost_extra.th_tokens': 'Tokens',
    'cost_extra.th_requests': 'Requests',
    'cost_extra.th_share': 'Share',

    // Logs Extra
    'logs_extra.title': 'Real-time Logs',
    'logs_extra.connected': 'Connected',
    'logs_extra.disconnected': 'Disconnected',
    'logs_extra.events': '{count} events',
    'logs_extra.resume': 'Resume',
    'logs_extra.pause': 'Pause',
    'logs_extra.jump_bottom': 'Jump to Bottom',
    'logs_extra.filter': 'Filter:',
    'logs_extra.clear': 'Clear',
    'logs_extra.paused_msg': 'Log stream paused',
    'logs_extra.waiting': 'Waiting for events...',

    // Doctor
    'doctor_extra.failed': 'Diagnostics failed',
    'doctor_extra.title': 'System Diagnostics',
    'doctor_extra.running': 'Running...',
    'doctor_extra.run_btn': 'Run Diagnostics',
    'doctor_extra.running_msg': 'Running diagnostics...',
    'doctor_extra.wait_msg': 'This may take a few seconds',
    'doctor_extra.status_ok': 'OK',
    'doctor_extra.status_warn': 'Warnings',
    'doctor_extra.status_error': 'Errors',
    'doctor_extra.issues_found': 'Issues Found',
    'doctor_extra.has_warns': 'Has Warnings',
    'doctor_extra.all_ok': 'All Good',
    'doctor_extra.hint': 'Click "Run Diagnostics" to check system status',

    // Devices
    'devices.unknown': 'Unknown',
    'devices.load_failed': 'Failed to load devices',
    'devices.revoke_failed': 'Failed to revoke device',
    'devices.title': 'Paired Devices',
    'devices.refresh': 'Refresh',
    'devices.empty': 'No paired devices',
    'devices.th_id': 'Device ID',
    'devices.th_paired_by': 'Paired By',
    'devices.th_created': 'Created At',
    'devices.th_last_seen': 'Last Seen',
    'devices.th_actions': 'Actions',
    'devices.confirm_revoke': 'Revoke?',
    'devices.yes': 'Yes',
    'devices.no': 'No',
    'devices.revoke': 'Revoke',

    // Profile
    'profile.fetch_failed': 'Failed to fetch user profile:',
    'profile.err_format': 'Only JPG, PNG, GIF, WebP are supported',
    'profile.err_size': 'File size cannot exceed 5MB',
    'profile.avatar_success': 'Avatar updated successfully',
    'profile.avatar_failed': 'Avatar upload failed',
    'profile.save_success': 'Saved successfully',
    'profile.save_failed': 'Save failed',
    'profile.not_set': 'Not set',
    'profile.male': 'Male',
    'profile.female': 'Female',
    'profile.title': 'Profile',
    'profile.avatar_alt': 'Avatar',
    'profile.uploading': 'Uploading...',
    'profile.change_avatar': 'Change Avatar',
    'profile.nickname': 'Nickname',
    'profile.nickname_pl': 'Set your nickname',
    'profile.phone': 'Phone',
    'profile.unbound': 'Not bound',
    'profile.gender': 'Gender',
    'profile.birthday': 'Birthday',
    'profile.bio': 'Bio',
    'profile.bio_pl': 'Introduce yourself',
    'profile.uuid': 'Huanxing ID',
    'profile.default_char': 'U',
  },

  tr: {
    // Navigation
    'nav.dashboard': 'Kontrol Paneli',
    'nav.agent': 'Ajan',
    'nav.tools': 'Araclar',
    'nav.cron': 'Zamanlanmis Gorevler',
    'nav.integrations': 'Entegrasyonlar',
    'nav.memory': 'Hafiza',
    'nav.devices': 'Cihazlar',
    'nav.config': 'Yapilandirma',
    'nav.cost': 'Maliyet Takibi',
    'nav.logs': 'Kayitlar',
    'nav.doctor': 'Doktor',

    // Dashboard
    'dashboard.title': 'Kontrol Paneli',
    'dashboard.provider': 'Saglayici',
    'dashboard.model': 'Model',
    'dashboard.uptime': 'Calisma Suresi',
    'dashboard.temperature': 'Sicaklik',
    'dashboard.gateway_port': 'Gecit Portu',
    'dashboard.locale': 'Yerel Ayar',
    'dashboard.memory_backend': 'Hafiza Motoru',
    'dashboard.paired': 'Eslestirilmis',
    'dashboard.channels': 'Kanallar',
    'dashboard.health': 'Saglik',
    'dashboard.status': 'Durum',
    'dashboard.overview': 'Genel Bakis',
    'dashboard.system_info': 'Sistem Bilgisi',
    'dashboard.quick_actions': 'Hizli Islemler',

    // Agent / Chat
    'agent.title': 'Ajan Sohbet',
    'agent.send': 'Gonder',
    'agent.placeholder': 'Bir mesaj yazin...',
    'agent.connecting': 'Baglaniyor...',
    'agent.connected': 'Bagli',
    'agent.disconnected': 'Baglanti Kesildi',
    'agent.reconnecting': 'Yeniden Baglaniyor...',
    'agent.thinking': 'Dusunuyor...',
    'agent.tool_call': 'Arac Cagrisi',
    'agent.tool_result': 'Arac Sonucu',

    // Tools
    'tools.title': 'Mevcut Araclar',
    'tools.name': 'Ad',
    'tools.description': 'Aciklama',
    'tools.parameters': 'Parametreler',
    'tools.search': 'Arac ara...',
    'tools.empty': 'Mevcut arac yok.',
    'tools.count': 'Toplam arac',

    // Cron
    'cron.title': 'Zamanlanmis Gorevler',
    'cron.add': 'Gorev Ekle',
    'cron.delete': 'Sil',
    'cron.enable': 'Etkinlestir',
    'cron.disable': 'Devre Disi Birak',
    'cron.name': 'Ad',
    'cron.command': 'Komut',
    'cron.schedule': 'Zamanlama',
    'cron.next_run': 'Sonraki Calistirma',
    'cron.last_run': 'Son Calistirma',
    'cron.last_status': 'Son Durum',
    'cron.enabled': 'Etkin',
    'cron.empty': 'Zamanlanmis gorev yok.',
    'cron.confirm_delete': 'Bu gorevi silmek istediginizden emin misiniz?',

    // Integrations
    'integrations.title': 'Entegrasyonlar',
    'integrations.available': 'Mevcut',
    'integrations.active': 'Aktif',
    'integrations.coming_soon': 'Yakinda',
    'integrations.category': 'Kategori',
    'integrations.status': 'Durum',
    'integrations.search': 'Entegrasyon ara...',
    'integrations.empty': 'Entegrasyon bulunamadi.',
    'integrations.activate': 'Etkinlestir',
    'integrations.deactivate': 'Devre Disi Birak',

    // Memory
    'memory.title': 'Hafiza Deposu',
    'memory.search': 'Hafizada ara...',
    'memory.add': 'Hafiza Kaydet',
    'memory.delete': 'Sil',
    'memory.key': 'Anahtar',
    'memory.content': 'Icerik',
    'memory.category': 'Kategori',
    'memory.timestamp': 'Zaman Damgasi',
    'memory.session': 'Oturum',
    'memory.score': 'Skor',
    'memory.empty': 'Hafiza kaydi bulunamadi.',
    'memory.confirm_delete': 'Bu hafiza kaydini silmek istediginizden emin misiniz?',
    'memory.all_categories': 'Tum Kategoriler',

    // Config
    'config.title': 'Yapilandirma',
    'config.save': 'Kaydet',
    'config.reset': 'Sifirla',
    'config.saved': 'Yapilandirma basariyla kaydedildi.',
    'config.error': 'Yapilandirma kaydedilemedi.',
    'config.loading': 'Yapilandirma yukleniyor...',
    'config.editor_placeholder': 'TOML yapilandirmasi...',

    // Cost
    'cost.title': 'Maliyet Takibi',
    'cost.session': 'Oturum Maliyeti',
    'cost.daily': 'Gunluk Maliyet',
    'cost.monthly': 'Aylik Maliyet',
    'cost.total_tokens': 'Toplam Token',
    'cost.request_count': 'Istekler',
    'cost.by_model': 'Modele Gore Maliyet',
    'cost.model': 'Model',
    'cost.tokens': 'Token',
    'cost.requests': 'Istekler',
    'cost.usd': 'Maliyet (USD)',

    // Logs
    'logs.title': 'Canli Kayitlar',
    'logs.clear': 'Temizle',
    'logs.pause': 'Duraklat',
    'logs.resume': 'Devam Et',
    'logs.filter': 'Kayitlari filtrele...',
    'logs.empty': 'Kayit girisi yok.',
    'logs.connected': 'Olay akisina baglandi.',
    'logs.disconnected': 'Olay akisi baglantisi kesildi.',

    // Doctor
    'doctor.title': 'Sistem Teshisleri',
    'doctor.run': 'Teshis Calistir',
    'doctor.running': 'Teshisler calistiriliyor...',
    'doctor.ok': 'Tamam',
    'doctor.warn': 'Uyari',
    'doctor.error': 'Hata',
    'doctor.severity': 'Ciddiyet',
    'doctor.category': 'Kategori',
    'doctor.message': 'Mesaj',
    'doctor.empty': 'Henuz teshis calistirilmadi.',
    'doctor.summary': 'Teshis Ozeti',

    // Auth / Pairing
    'auth.pair': 'Cihaz Esle',
    'auth.pairing_code': 'Eslestirme Kodu',
    'auth.pair_button': 'Esle',
    'auth.logout': 'Cikis Yap',
    'auth.pairing_success': 'Eslestirme basarili!',
    'auth.pairing_failed': 'Eslestirme basarisiz. Lutfen tekrar deneyin.',
    'auth.enter_code': 'Ajana baglanmak icin eslestirme kodunuzu girin.',

    // Common
    'common.loading': 'Yukleniyor...',
    'common.error': 'Bir hata olustu.',
    'common.retry': 'Tekrar Dene',
    'common.cancel': 'Iptal',
    'common.confirm': 'Onayla',
    'common.save': 'Kaydet',
    'common.delete': 'Sil',
    'common.edit': 'Duzenle',
    'common.close': 'Kapat',
    'common.yes': 'Evet',
    'common.no': 'Hayir',
    'common.search': 'Ara...',
    'common.no_data': 'Veri mevcut degil.',
    'common.refresh': 'Yenile',
    'common.back': 'Geri',
    'common.actions': 'Islemler',
    'common.name': 'Ad',
    'common.description': 'Aciklama',
    'common.status': 'Durum',
    'common.created': 'Olusturulma',
    'common.updated': 'Guncellenme',

    // Health
    'health.title': 'Sistem Sagligi',
    'health.component': 'Bilesen',
    'health.status': 'Durum',
    'health.last_ok': 'Son Basarili',
    'health.last_error': 'Son Hata',
    'health.restart_count': 'Yeniden Baslatmalar',
    'health.pid': 'Islem Kimligi',
    'health.uptime': 'Calisma Suresi',
    'health.updated_at': 'Son Guncelleme',
  },

  'zh-CN': {
    // Navigation
    'nav.dashboard': '仪表盘',
    'nav.agent': '智能体',
    'nav.tools': '工具',
    'nav.cron': '定时任务',
    'nav.integrations': '集成',
    'nav.memory': '记忆',
    'nav.devices': '设备',
    'nav.config': '配置',
    'nav.cost': '成本追踪',
    'nav.logs': '日志',
    'nav.doctor': '诊断',

    // Dashboard
    'dashboard.title': '仪表盘',
    'dashboard.provider': '提供商',
    'dashboard.model': '模型',
    'dashboard.uptime': '运行时长',
    'dashboard.temperature': '温度',
    'dashboard.gateway_port': '网关端口',
    'dashboard.locale': '语言区域',
    'dashboard.memory_backend': '记忆后端',
    'dashboard.paired': '已配对',
    'dashboard.channels': '渠道',
    'dashboard.health': '健康状态',
    'dashboard.status': '状态',
    'dashboard.overview': '总览',
    'dashboard.system_info': '系统信息',
    'dashboard.quick_actions': '快捷操作',

    // Agent / Chat
    'agent.title': '智能体聊天',
    'agent.send': '发送',
    'agent.placeholder': '输入消息...',
    'agent.connecting': '连接中...',
    'agent.connected': '已连接',
    'agent.disconnected': '已断开连接',
    'agent.reconnecting': '重连中...',
    'agent.thinking': '思考中...',
    'agent.tool_call': '工具调用',
    'agent.tool_result': '工具结果',

    // Tools
    'tools.title': '可用工具',
    'tools.name': '名称',
    'tools.description': '描述',
    'tools.parameters': '参数',
    'tools.search': '搜索工具...',
    'tools.empty': '暂无可用工具。',
    'tools.count': '工具总数',

    // Cron
    'cron.title': '定时任务',
    'cron.add': '添加任务',
    'cron.delete': '删除',
    'cron.enable': '启用',
    'cron.disable': '禁用',
    'cron.name': '名称',
    'cron.command': '命令',
    'cron.schedule': '计划',
    'cron.next_run': '下次运行',
    'cron.last_run': '上次运行',
    'cron.last_status': '上次状态',
    'cron.enabled': '已启用',
    'cron.empty': '暂无定时任务。',
    'cron.confirm_delete': '确定要删除此任务吗？',

    // Integrations
    'integrations.title': '集成',
    'integrations.available': '可用',
    'integrations.active': '已激活',
    'integrations.coming_soon': '即将推出',
    'integrations.category': '分类',
    'integrations.status': '状态',
    'integrations.search': '搜索集成...',
    'integrations.empty': '未找到集成。',
    'integrations.activate': '激活',
    'integrations.deactivate': '停用',

    // Memory
    'memory.title': '记忆存储',
    'memory.search': '搜索记忆...',
    'memory.add': '存储记忆',
    'memory.delete': '删除',
    'memory.key': '键',
    'memory.content': '内容',
    'memory.category': '分类',
    'memory.timestamp': '时间戳',
    'memory.session': '会话',
    'memory.score': '评分',
    'memory.empty': '未找到记忆条目。',
    'memory.confirm_delete': '确定要删除此记忆条目吗？',
    'memory.all_categories': '全部分类',

    // Config
    'config.title': '配置',
    'config.save': '保存',
    'config.reset': '重置',
    'config.saved': '配置保存成功。',
    'config.error': '配置保存失败。',
    'config.loading': '配置加载中...',
    'config.editor_placeholder': 'TOML 配置...',

    // Cost
    'cost.title': '成本追踪',
    'cost.session': '会话成本',
    'cost.daily': '每日成本',
    'cost.monthly': '每月成本',
    'cost.total_tokens': 'Token 总数',
    'cost.request_count': '请求数',
    'cost.by_model': '按模型统计成本',
    'cost.model': '模型',
    'cost.tokens': 'Token',
    'cost.requests': '请求',
    'cost.usd': '成本（USD）',

    // Logs
    'logs.title': '实时日志',
    'logs.clear': '清空',
    'logs.pause': '暂停',
    'logs.resume': '继续',
    'logs.filter': '筛选日志...',
    'logs.empty': '暂无日志条目。',
    'logs.connected': '已连接到事件流。',
    'logs.disconnected': '与事件流断开连接。',

    // Doctor
    'doctor.title': '系统诊断',
    'doctor.run': '运行诊断',
    'doctor.running': '正在运行诊断...',
    'doctor.ok': '正常',
    'doctor.warn': '警告',
    'doctor.error': '错误',
    'doctor.severity': '严重级别',
    'doctor.category': '分类',
    'doctor.message': '消息',
    'doctor.empty': '尚未运行诊断。',
    'doctor.summary': '诊断摘要',

    // Auth / Pairing
    'auth.pair': '设备配对',
    'auth.pairing_code': '配对码',
    'auth.pair_button': '配对',
    'auth.logout': '退出登录',
    'auth.pairing_success': '配对成功！',
    'auth.pairing_failed': '配对失败，请重试。',
    'auth.enter_code': '输入配对码以连接到智能体。',

    // Common
    'common.loading': '加载中...',
    'common.error': '发生错误。',
    'common.retry': '重试',
    'common.cancel': '取消',
    'common.confirm': '确认',
    'common.save': '保存',
    'common.delete': '删除',
    'common.edit': '编辑',
    'common.close': '关闭',
    'common.yes': '是',
    'common.no': '否',
    'common.search': '搜索...',
    'common.no_data': '暂无数据。',
    'common.refresh': '刷新',
    'common.back': '返回',
    'common.actions': '操作',
    'common.name': '名称',
    'common.description': '描述',
    'common.status': '状态',
    'common.created': '创建时间',
    'common.updated': '更新时间',

    // Health
    'health.title': '系统健康',
    'health.component': '组件',
    'health.status': '状态',
    'health.last_ok': '最近正常',
    'health.last_error': '最近错误',
    'health.restart_count': '重启次数',
    'health.pid': '进程 ID',
    'health.uptime': '运行时长',
    'health.updated_at': '最后更新',

    // Settings Panel
    'settings.title': '设置中心',
    'settings.general': '常规',
    'settings.system': '系统',
    'settings.about': '关于',
    'settings.engine': 'AI 引擎',
    'settings.about_app': '关于唤星',

    // Dashboard Extra
    'dash.load_fail': '加载仪表盘失败',
    'dash.fail_title': '加载失败',
    'dash.subtitle': '唤星运行控制台',
    'dash.desc': '实时运行状态、费用统计、渠道连接状态一览',
    'dash.running': '运行中',
    'dash.unpaired': '未配对',
    'dash.unknown': '未知',
    'dash.since_restart': '自上次重启',
    'dash.device_paired': '设备已配对',
    'dash.no_paired_device': '无配对设备',
    'dash.cost_title': '费用统计',
    'dash.cost_subtitle': '会话、日、月度运行费用',
    'dash.session': '本次会话',
    'dash.today': '今日',
    'dash.this_month': '本月',
    'dash.total_tokens': '总 Token 数',
    'dash.request_count': '请求数',
    'dash.channels_title': '渠道状态',
    'dash.channels_subtitle': '接入渠道和连接状态',
    'dash.no_channels': '暂无接入渠道',
    'dash.connected': '已连接',
    'dash.disconnected': '未连接',
    'dash.health_title': '组件健康',
    'dash.health_subtitle': '运行时心跳和组件状态',
    'dash.no_health_data': '暂无组件健康数据',

    // Integrations
    'integ.active': '已启用',
    'integ.available': '可用',
    'integ.coming_soon': '敬请期待',
    'integ.no_integrations': '未找到集成。',
    'integ.current_model': '当前模型',
    'integ.custom_model_hint': '要配置自定义模型，请使用“编辑凭证”。',
    'integ.default_configured': '已配置默认提供商',
    'integ.provider_configured': '已配置提供商',
    'integ.creds_configured': '已配置凭据',
    'integ.creds_not_configured': '未配置凭据',
    'integ.edit_keys': '编辑凭据',
    'integ.configure': '配置',
    'integ.configure_title': '配置 {name}',
    'integ.enter_update': '仅输入您想要更新的字段。',
    'integ.enter_required': '输入必要的字段以配置此集成。',
    'integ.saving_updates_default': '保存此页不仅会更新凭据，还会将您的默认 AI 提供商切换为',
    'integ.adv_settings': '如需进行高级设置，请进入',
    'integ.configuration': '配置中心',
    'integ.configured_badge': '已配置',
    'integ.select_recommended': '选择推荐模型',
    'integ.custom_model': '自定义模型...',
    'integ.clear_current': '清除当前模型配置',
    'integ.current_value': '当前值：',
    'integ.replace_current': '输入新值覆盖当前值',
    'integ.enter_value': '输入值',
    'integ.leave_empty': '输入新值，或者留空以保持当前值',
    'integ.optional': '可选填',
    'integ.cancel': '取消',
    'integ.save_config': '保存配置',
    'integ.saving': '保存中...',
    'integ.apply': '应用',

    // Memory Extra
    'mem.load_fail': '加载失败',
    'mem.required': 'Key 和内容为必填项',
    'mem.save_fail': '保存记忆失败',
    'mem.del_fail': '删除失败',
    'mem.title': '记忆管理',
    'mem.add': '添加记忆',
    'mem.search_pl': '搜索记忆...',
    'mem.all_cat': '全部分类',
    'mem.search': '搜索',
    'mem.key_pl': '如 user_preferences',
    'mem.content': '内容',
    'mem.content_pl': '记忆内容...',
    'mem.cat_opt': '分类（可选）',
    'mem.cat_pl': '如 preferences, context',
    'mem.cancel': '取消',
    'mem.saving': '保存中...',
    'mem.save': '保存',
    'mem.no_entries': '暂无记忆条目',
    'mem.th_key': 'Key',
    'mem.th_content': '内容',
    'mem.th_cat': '分类',
    'mem.th_time': '时间',
    'mem.th_actions': '操作',
    'mem.del_confirm': '删除？',
    'mem.yes': '是',
    'mem.no': '否',

    // Config Extra
    'config.editor_title': '配置编辑',
    'config.form': '表单',
    'config.raw': '原始',
    'config.saving': '保存中...',
    'config.hidden_title': '敏感字段已隐藏',
    'config.hidden_desc_form': '被隐藏的字段显示为“已配置（已隐藏）”。保持不变则保留原值，输入新值则更新。',
    'config.hidden_desc_raw': 'API 密钥、令牌和密码已隐藏。要更新被隐藏的字段，请用新值替换整个隐藏值。',
    'config.select_placeholder': '请选择...',

    // Engine Extra
    'engine.title': 'AI 引擎',
    'engine.refresh': '刷新',
    'engine.running': '运行中',
    'engine.stopped': '已停止',
    'engine.starting': '启动中...',
    'engine.start': '启动',
    'engine.restarting': '重启中...',
    'engine.restart': '重启',
    'engine.stopping': '停止中...',
    'engine.stop': '停止',
    'engine.model': '模型',
    'engine.uptime': '运行时间',
    'engine.memory_backend': '记忆后端',
    'engine.restarted_count': '已自动重启 {count} 次',
    'engine.quick_config': '快捷配置',
    'engine.loading_config': '加载配置中...',
    'engine.config_helper': '仅 Tauri 桌面端可修改配置。开发模式请手动编辑 config.toml',
    'engine.logs_title': '运行日志',
    'engine.lines': '{count} 行',
    'engine.auto_scroll': '自动滚动',
    'engine.clear_logs': '清空日志',
    'engine.export_logs': '导出日志',
    'engine.waiting_logs': '等待日志...',
    'engine.not_running': '引擎未运行',
    'engine.default_model': '默认模型',
    'engine.not_set': '（未设置）',
    'engine.temperature': '温度',
    'engine.precise': '精确 0',
    'engine.balanced': '平衡 1',
    'engine.creative': '创意 2',
    'engine.autonomy': '自主级别',
    'engine.supervised': '监督模式 — 高风险操作需确认',
    'engine.semi': '半自主 — 仅文件删除需确认',
    'engine.full': '全自主 — 所有操作自动执行',
    'engine.reset': '重置',
    'engine.save_restart': '保存并重启',

    // Tools Extra
    'tools.load_failed': '加载失败',
    'tools.search_pl': '搜索工具...',
    'tools.agent_tools': 'Agent 工具',
    'tools.no_match': '未找到匹配的工具',
    'tools.schema': '参数 SCHEMA',
    'tools.cli_tools': 'CLI 工具',
    'tools.th_name': '名称',
    'tools.th_path': '路径',
    'tools.th_version': '版本',
    'tools.th_category': '类别',

    // Cron Extra
    'cron_extra.required': '调度表达式和命令为必填项',
    'cron_extra.add_failed': '添加任务失败',
    'cron_extra.del_failed': '删除失败',
    'cron_extra.title': '定时任务',
    'cron_extra.add_btn': '添加任务',
    'cron_extra.add_title': '添加定时任务',
    'cron_extra.name_opt': '名称（可选）',
    'cron_extra.name_pl': '如 每日清理',
    'cron_extra.schedule': '调度表达式',
    'cron_extra.schedule_pl': '如 0 0 * * * (cron 表达式)',
    'cron_extra.command': '命令',
    'cron_extra.command_pl': '如 cleanup --older-than 7d',
    'cron_extra.cancel': '取消',
    'cron_extra.adding': '添加中...',
    'cron_extra.add': '添加',
    'cron_extra.no_jobs': '暂无定时任务',
    'cron_extra.th_id': 'ID',
    'cron_extra.th_name': '名称',
    'cron_extra.th_cmd': '命令',
    'cron_extra.th_next': '下次运行',
    'cron_extra.th_status': '状态',
    'cron_extra.th_enabled': '启用',
    'cron_extra.th_actions': '操作',
    'cron_extra.enabled': '已启用',
    'cron_extra.disabled': '已禁用',
    'cron_extra.del_confirm': '删除？',
    'cron_extra.yes': '是',
    'cron_extra.no': '否',

    // Cost Extra
    'cost_extra.session': '本次会话',
    'cost_extra.daily': '今日费用',
    'cost_extra.monthly': '本月费用',
    'cost_extra.requests': '总请求数',
    'cost_extra.token_stats': 'Token 统计',
    'cost_extra.total_tokens': '总 Token 数',
    'cost_extra.avg_tokens': '平均 Token/请求',
    'cost_extra.cost_per_1k': '每千 Token 费用',
    'cost_extra.model_breakdown': '模型明细',
    'cost_extra.no_data': '暂无模型数据',
    'cost_extra.th_model': '模型',
    'cost_extra.th_cost': '费用',
    'cost_extra.th_tokens': 'Token',
    'cost_extra.th_requests': '请求',
    'cost_extra.th_share': '占比',

    // Logs Extra
    'logs_extra.title': '实时日志',
    'logs_extra.connected': '已连接',
    'logs_extra.disconnected': '未连接',
    'logs_extra.events': '{count} 条事件',
    'logs_extra.resume': '继续',
    'logs_extra.pause': '暂停',
    'logs_extra.jump_bottom': '跳到底部',
    'logs_extra.filter': '筛选：',
    'logs_extra.clear': '清除',
    'logs_extra.paused_msg': '日志流已暂停',
    'logs_extra.waiting': '等待事件...',

    // Doctor
    'doctor_extra.failed': '诊断失败',
    'doctor_extra.title': '系统诊断',
    'doctor_extra.running': '诊断中...',
    'doctor_extra.run_btn': '运行诊断',
    'doctor_extra.running_msg': '正在运行诊断...',
    'doctor_extra.wait_msg': '可能需要几秒钟',
    'doctor_extra.status_ok': '正常',
    'doctor_extra.status_warn': '警告',
    'doctor_extra.status_error': '错误',
    'doctor_extra.issues_found': '发现问题',
    'doctor_extra.has_warns': '有警告',
    'doctor_extra.all_ok': '全部正常',
    'doctor_extra.hint': '点击"运行诊断"检查系统状态',

    // Devices
    'devices.unknown': '未知',
    'devices.load_failed': '加载设备列表失败',
    'devices.revoke_failed': '撤销设备失败',
    'devices.title': '已配对设备',
    'devices.refresh': '刷新',
    'devices.empty': '暂无配对设备',
    'devices.th_id': '设备 ID',
    'devices.th_paired_by': '配对方式',
    'devices.th_created': '创建时间',
    'devices.th_last_seen': '最后在线',
    'devices.th_actions': '操作',
    'devices.confirm_revoke': '确认撤销？',
    'devices.yes': '是',
    'devices.no': '否',
    'devices.revoke': '撤销',

    // Profile
    'profile.fetch_failed': '获取用户资料失败:',
    'profile.err_format': '仅支持 JPG、PNG、GIF、WebP 格式',
    'profile.err_size': '文件大小不能超过 5MB',
    'profile.avatar_success': '头像更新成功',
    'profile.avatar_failed': '头像上传失败',
    'profile.save_success': '保存成功',
    'profile.save_failed': '保存失败',
    'profile.not_set': '未设置',
    'profile.male': '男',
    'profile.female': '女',
    'profile.title': '个人资料',
    'profile.avatar_alt': '头像',
    'profile.uploading': '上传中...',
    'profile.change_avatar': '更换头像',
    'profile.nickname': '昵称',
    'profile.nickname_pl': '设置你的昵称',
    'profile.phone': '手机号',
    'profile.unbound': '未绑定',
    'profile.gender': '性别',
    'profile.birthday': '生日',
    'profile.bio': '个人简介',
    'profile.bio_pl': '介绍一下自己',
    'profile.uuid': '唤星 ID',
    'profile.default_char': '用',
  },
  ja: {},
  ru: {},
  fr: {},
  vi: {},
  el: {},
};

// ---------------------------------------------------------------------------
// Current locale state
// ---------------------------------------------------------------------------

let currentLocale: Locale = 'en';

export function getLocale(): Locale {
  return currentLocale;
}

export function setLocale(locale: Locale): void {
  currentLocale = locale;
}

// ---------------------------------------------------------------------------
// Translation function
// ---------------------------------------------------------------------------

/**
 * Translate a key using the current locale. Returns the key itself if no
 * translation is found.
 */
export function t(key: string, variables?: Record<string, string | number>): string {
  let text = translations[currentLocale]?.[key] ?? translations.en[key] ?? key;
  if (variables) {
    for (const [k, v] of Object.entries(variables)) {
      text = text.replace(new RegExp(`\\{${k}\\}`, 'g'), String(v));
    }
  }
  return text;
}

/**
 * Get the translation for a specific locale. Falls back to English, then to the
 * raw key.
 */
export function tLocale(key: string, locale: Locale, variables?: Record<string, string | number>): string {
  let text = translations[locale]?.[key] ?? translations.en[key] ?? key;
  if (variables) {
    for (const [k, v] of Object.entries(variables)) {
      text = text.replace(new RegExp(`\\{${k}\\}`, 'g'), String(v));
    }
  }
  return text;
}

// ---------------------------------------------------------------------------
// React hook
// ---------------------------------------------------------------------------

export function coerceLocale(locale: string | undefined): Locale {
  if (!locale) return 'en';
  if (KNOWN_LOCALES.includes(locale as Locale)) return locale as Locale;

  const lowered = locale.toLowerCase();
  if (lowered.startsWith('tr')) return 'tr';
  if (lowered === 'zh' || lowered.startsWith('zh-')) return 'zh-CN';
  if (lowered === 'ja' || lowered.startsWith('ja-')) return 'ja';
  if (lowered === 'ru' || lowered.startsWith('ru-')) return 'ru';
  if (lowered === 'fr' || lowered.startsWith('fr-')) return 'fr';
  if (lowered === 'vi' || lowered.startsWith('vi-')) return 'vi';
  if (lowered === 'el' || lowered.startsWith('el-')) return 'el';
  return 'en';
}

/**
 * React hook that fetches the locale from /api/status on mount and keeps the
 * i18n module in sync. Returns the current locale and a `t` helper bound to it.
 */
export function useLocale(): { locale: Locale; t: (key: string, variables?: Record<string, string | number>) => string } {
  const [locale, setLocaleState] = useState<Locale>(currentLocale);

  useEffect(() => {
    let cancelled = false;

    getStatus()
      .then((status) => {
        if (cancelled) return;
        const detected = coerceLocale(status.locale);
        setLocale(detected);
        setLocaleState(detected);
      })
      .catch(() => {
        // Keep default locale on error
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return {
    locale,
    t: (key: string, variables?: Record<string, string | number>) => tLocale(key, locale, variables),
  };
}
