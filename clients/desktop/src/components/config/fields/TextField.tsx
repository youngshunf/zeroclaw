import { useState } from 'react';
import { Eye, EyeOff, Lock } from 'lucide-react';
import type { FieldProps } from '../types';
import { useLocaleContext } from '@/App';
import { t_config } from '../configTranslations';
import { Input } from '../../ui/Input';

export default function TextField({ field, value, onChange, isMasked }: FieldProps) {
  const { locale } = useLocaleContext();
  const [showPassword, setShowPassword] = useState(false);
  const isPassword = field.type === 'password';
  const strValue = isMasked ? '' : ((value as string) ?? '');

  return (
    <div className="relative">
      <Input
        type={isPassword && !showPassword ? 'password' : 'text'}
        value={strValue}
        onChange={(e) => onChange(e.target.value)}
        placeholder={isMasked ? t_config('Configured (masked)', locale) : field.description ?? ''}
        className="pr-16"
      />
      <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1">
        {isMasked && (
          <Lock className="h-3.5 w-3.5 text-yellow-500" />
        )}
        {isPassword && (
          <button
            type="button"
            onClick={() => setShowPassword(!showPassword)}
            className="p-1 text-gray-400 hover:text-gray-200 transition-colors"
          >
            {showPassword ? (
              <EyeOff className="h-3.5 w-3.5" />
            ) : (
              <Eye className="h-3.5 w-3.5" />
            )}
          </button>
        )}
      </div>
    </div>
  );
}
