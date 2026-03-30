/**
 * 个人资料页面
 * 展示和编辑用户信息：头像、昵称、性别、生日、简介等
 */
import { useState, useRef, useEffect, useCallback } from 'react';
import { Camera, Save, ArrowLeft, Check, X } from 'lucide-react';
import { getHuanxingSession, type HuanxingSession } from '../../config';
import { uploadAvatar, updateAvatar, updateProfile, getUserProfile } from '../../lib/huanxing-api';
import AvatarCropDialog from './AvatarCropDialog';
import { t } from '../../../lib/i18n';
import { useLocaleContext } from '../../../App';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../../components/ui/Select';

interface UserProfile {
  nickname: string;
  phone: string;
  email: string;
  avatar: string;
  gender: string;
  birthday: string;
  bio: string;
  uuid: string;
}

export default function ProfilePage() {
  const session = getHuanxingSession();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const { locale } = useLocaleContext();

  const [profile, setProfile] = useState<UserProfile>({
    nickname: session?.user?.nickname || '',
    phone: session?.user?.phone || '',
    email: (session?.user as any)?.email || '',
    avatar: session?.user?.avatar || '',
    gender: '',
    birthday: '',
    bio: '',
    uuid: session?.user?.uuid || '',
  });

  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);
  const [cropImage, setCropImage] = useState<string | null>(null);
  const [uploading, setUploading] = useState(false);
  const [editField, setEditField] = useState<string | null>(null);
  const [editValue, setEditValue] = useState('');

  // 加载完整用户资料
  useEffect(() => {
    if (!session?.accessToken) return;
    getUserProfile(session.accessToken)
      .then((resp: any) => {
        const u = resp.data;
        if (u) {
          setProfile((prev) => ({
            ...prev,
            nickname: u.nickname || prev.nickname,
            phone: u.phone || prev.phone,
            email: u.email || prev.email,
            avatar: u.avatar || prev.avatar,
            gender: u.gender || '',
            birthday: u.birthday || '',
            bio: u.bio || '',
            uuid: u.uuid || prev.uuid,
          }));
        }
      })
      .catch((err: Error) => console.warn(`${t('profile.fetch_failed')} ${err}`));
  }, []);

  // 头像选择
  const handleAvatarClick = () => fileInputRef.current?.click();

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    // 校验文件类型和大小
    if (!['image/jpeg', 'image/png', 'image/gif', 'image/webp'].includes(file.type)) {
      showMessage('error', t('profile.err_format'));
      return;
    }
    if (file.size > 5 * 1024 * 1024) {
      showMessage('error', t('profile.err_size'));
      return;
    }
    const reader = new FileReader();
    reader.onload = () => setCropImage(reader.result as string);
    reader.readAsDataURL(file);
    // 清空 input 以便重复选择同一文件
    e.target.value = '';
  };

  // 裁剪完成，上传
  const handleCropComplete = useCallback(async (blob: Blob) => {
    setCropImage(null);
    if (!session?.accessToken) return;
    setUploading(true);
    try {
      const url = await uploadAvatar(session.accessToken, blob, 'avatar.png');
      if (url) {
        await updateAvatar(session.accessToken, url);
        setProfile((prev) => ({ ...prev, avatar: url }));
        // 更新 localStorage 中的 session
        updateLocalSession({ avatar: url });
        showMessage('success', t('profile.avatar_success'));
      }
    } catch (err: any) {
      showMessage('error', err.message || t('profile.avatar_failed'));
    } finally {
      setUploading(false);
    }
  }, [session]);

  // 开始编辑字段
  const startEdit = (field: string, value: string) => {
    setEditField(field);
    setEditValue(value);
  };

  // 取消编辑
  const cancelEdit = () => {
    setEditField(null);
    setEditValue('');
  };

  // 保存单个字段
  const saveField = async (field: string) => {
    if (!session?.accessToken) return;
    setSaving(true);
    try {
      await updateProfile(session.accessToken, { [field]: editValue });
      setProfile((prev) => ({ ...prev, [field]: editValue }));
      if (field === 'nickname') {
        updateLocalSession({ nickname: editValue });
      }
      showMessage('success', t('profile.save_success'));
      setEditField(null);
    } catch (err: any) {
      showMessage('error', err.message || t('profile.save_failed'));
    } finally {
      setSaving(false);
    }
  };

  const showMessage = (type: 'success' | 'error', text: string) => {
    setMessage({ type, text });
    setTimeout(() => setMessage(null), 3000);
  };

  // 更新 localStorage 中的 session
  const updateLocalSession = (updates: Partial<HuanxingSession['user']>) => {
    try {
      const raw = localStorage.getItem('huanxing_session');
      if (!raw) return;
      const s = JSON.parse(raw);
      s.user = { ...s.user, ...updates };
      localStorage.setItem('huanxing_session', JSON.stringify(s));
    } catch {}
  };

  const genderOptions = [
    { value: 'none', label: t('profile.not_set') },
    { value: 'male', label: t('profile.male') },
    { value: 'female', label: t('profile.female') },
  ];

  const avatarChar = profile.nickname?.charAt(0) || profile.phone?.charAt(0) || t('profile.default_char');

  return (
    <div className="hx-profile-page">
      {/* Header */}
      <div className="hx-profile-header">
        <h2>{t('profile.title')}</h2>
      </div>

      {/* Toast */}
      {message && (
        <div className={`hx-profile-toast hx-profile-toast-${message.type}`}>
          {message.text}
        </div>
      )}

      {/* Avatar section */}
      <div className="hx-profile-avatar-section">
        <div className="hx-profile-avatar-wrap" onClick={handleAvatarClick}>
          {profile.avatar ? (
            <img className="hx-profile-avatar" src={profile.avatar} alt={t('profile.avatar_alt')} />
          ) : (
            <div className="hx-profile-avatar hx-profile-avatar-placeholder">
              {avatarChar}
            </div>
          )}
          <div className="hx-profile-avatar-overlay">
            <Camera size={20} />
            <span>{uploading ? t('profile.uploading') : t('profile.change_avatar')}</span>
          </div>
        </div>
        <input
          ref={fileInputRef}
          type="file"
          accept="image/jpeg,image/png,image/gif,image/webp"
          className="hidden"
          onChange={handleFileChange}
        />
      </div>

      {/* Profile fields */}
      <div className="hx-profile-fields">
        {/* 昵称 */}
        <ProfileField
          label={t('profile.nickname')}
          value={profile.nickname}
          field="nickname"
          editField={editField}
          editValue={editValue}
          saving={saving}
          onStartEdit={startEdit}
          onSave={saveField}
          onCancel={cancelEdit}
          onChange={setEditValue}
          placeholder={t('profile.nickname_pl')}
        />

        {/* 手机号 */}
        <div className="hx-profile-field">
          <label>{t('profile.phone')}</label>
          <div className="hx-profile-field-value">
            <span>{profile.phone || t('profile.unbound')}</span>
          </div>
        </div>

        {/* 性别 */}
        <div className="hx-profile-field">
          <label>{t('profile.gender')}</label>
          <div className="hx-profile-field-value">
            {editField === 'gender' ? (
              <div className="hx-profile-field-edit flex items-center gap-1.5">
                <div className="flex-1 min-w-[120px]">
                  <Select value={editValue || 'none'} onValueChange={(val: string) => setEditValue(val === 'none' ? '' : val)}>
                    <SelectTrigger className="hx-profile-select w-full min-w-[120px]">
                      <SelectValue placeholder={t('profile.not_set')} />
                    </SelectTrigger>
                    <SelectContent>
                      {genderOptions.map((opt) => (
                        <SelectItem key={opt.value} value={opt.value}>
                          {opt.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
                <button className="hx-profile-field-btn" onClick={() => saveField('gender')} disabled={saving}>
                  <Check size={14} />
                </button>
                <button className="hx-profile-field-btn" onClick={cancelEdit}>
                  <X size={14} />
                </button>
              </div>
            ) : (
              <span
                className="hx-profile-field-editable"
                onClick={() => startEdit('gender', profile.gender)}
              >
                {genderOptions.find((o) => o.value === profile.gender)?.label || t('profile.not_set')}
              </span>
            )}
          </div>
        </div>

        {/* 生日 */}
        <div className="hx-profile-field">
          <label>{t('profile.birthday')}</label>
          <div className="hx-profile-field-value">
            {editField === 'birthday' ? (
              <div className="hx-profile-field-edit">
                <input
                  type="date"
                  className="hx-profile-input"
                  value={editValue}
                  onChange={(e) => setEditValue(e.target.value)}
                />
                <button className="hx-profile-field-btn" onClick={() => saveField('birthday')} disabled={saving}>
                  <Check size={14} />
                </button>
                <button className="hx-profile-field-btn" onClick={cancelEdit}>
                  <X size={14} />
                </button>
              </div>
            ) : (
              <span
                className="hx-profile-field-editable"
                onClick={() => startEdit('birthday', profile.birthday)}
              >
                {profile.birthday || t('profile.not_set')}
              </span>
            )}
          </div>
        </div>

        {/* 个人简介 */}
        <ProfileField
          label={t('profile.bio')}
          value={profile.bio}
          field="bio"
          editField={editField}
          editValue={editValue}
          saving={saving}
          onStartEdit={startEdit}
          onSave={saveField}
          onCancel={cancelEdit}
          onChange={setEditValue}
          placeholder={t('profile.bio_pl')}
          multiline
        />

        {/* UUID */}
        <div className="hx-profile-field">
          <label>{t('profile.uuid')}</label>
          <div className="hx-profile-field-value">
            <span className="hx-profile-field-mono">{profile.uuid || '-'}</span>
          </div>
        </div>
      </div>

      {/* Avatar crop dialog */}
      {cropImage && (
        <AvatarCropDialog
          imageSrc={cropImage}
          onCropComplete={handleCropComplete}
          onClose={() => setCropImage(null)}
        />
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// 可编辑字段子组件
// ---------------------------------------------------------------------------

interface ProfileFieldProps {
  label: string;
  value: string;
  field: string;
  editField: string | null;
  editValue: string;
  saving: boolean;
  onStartEdit: (field: string, value: string) => void;
  onSave: (field: string) => void;
  onCancel: () => void;
  onChange: (value: string) => void;
  placeholder?: string;
  multiline?: boolean;
}

function ProfileField({
  label, value, field, editField, editValue, saving,
  onStartEdit, onSave, onCancel, onChange, placeholder, multiline,
}: ProfileFieldProps) {
  return (
    <div className="hx-profile-field">
      <label>{label}</label>
      <div className="hx-profile-field-value">
        {editField === field ? (
          <div className="hx-profile-field-edit">
            {multiline ? (
              <textarea
                className="hx-profile-textarea"
                value={editValue}
                onChange={(e) => onChange(e.target.value)}
                placeholder={placeholder}
                rows={3}
                autoFocus
              />
            ) : (
              <input
                className="hx-profile-input"
                value={editValue}
                onChange={(e) => onChange(e.target.value)}
                placeholder={placeholder}
                autoFocus
                onKeyDown={(e) => e.key === 'Enter' && onSave(field)}
              />
            )}
            <button className="hx-profile-field-btn" onClick={() => onSave(field)} disabled={saving}>
              <Check size={14} />
            </button>
            <button className="hx-profile-field-btn" onClick={onCancel}>
              <X size={14} />
            </button>
          </div>
        ) : (
          <span
            className="hx-profile-field-editable"
            onClick={() => onStartEdit(field, value)}
          >
            {value || placeholder || t('profile.not_set')}
          </span>
        )}
      </div>
    </div>
  );
}
