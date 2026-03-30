import type { FieldProps } from '../types';
import { Input } from '../../ui/Input';

export default function NumberField({ field, value, onChange }: FieldProps) {
  const numValue = value === undefined || value === null || value === '' ? '' : Number(value);

  return (
    <Input
      type="number"
      value={numValue}
      onChange={(e) => {
        const raw = e.target.value;
        if (raw === '') {
          onChange(undefined);
          return;
        }
        const n = Number(raw);
        if (!isNaN(n)) {
          onChange(n);
        }
      }}
      onBlur={(e) => {
        if (field.step !== undefined && field.step < 1) {
          return;
        }
        const raw = e.target.value;
        if (raw === '') {
          return;
        }
        const n = Number(raw);
        if (!isNaN(n)) {
          onChange(Math.floor(n));
        }
      }}
      min={field.min}
      max={field.max}
      step={field.step ?? 1}
      placeholder={field.description ?? ''}
      className=""
    />
  );
}
