import type { FieldProps } from '../types';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/Select';
import { t } from '@/lib/i18n';

export default function SelectField({ field, value, onChange }: FieldProps) {
  const strValue = (value as string) ?? '';

  return (
    <Select value={strValue || 'none'} onValueChange={(v) => onChange(v === 'none' ? '' : v)}>
      <SelectTrigger className="w-full bg-gray-800 border-gray-700 text-white min-h-[38px]">
        <SelectValue placeholder={t('config.select_placeholder')} />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="none">{t('config.select_placeholder')}</SelectItem>
        {field.options?.map((opt) => (
          <SelectItem key={opt.value} value={opt.value}>
            {opt.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
