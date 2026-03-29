import { useMemo } from 'react';
import { useLocaleContext } from '@/App';
import { CONFIG_SECTIONS } from './configSections';
import { t_config } from './configTranslations';
import type { SectionDef } from './types';

export function useTranslatedConfigSections(): SectionDef[] {
  const { locale } = useLocaleContext();

  return useMemo(() => {
    return CONFIG_SECTIONS.map((section) => ({
      ...section,
      title: t_config(section.title, locale),
      description: section.description ? t_config(section.description, locale) : undefined,
      fields: section.fields.map((field) => ({
        ...field,
        label: t_config(field.label, locale),
        description: field.description ? t_config(field.description, locale) : undefined,
        tagPlaceholder: field.tagPlaceholder ? t_config(field.tagPlaceholder, locale) : undefined,
        options: field.options
          ? field.options.map((opt) => ({
              ...opt,
              label: t_config(opt.label, locale),
            }))
          : undefined,
      })),
    }));
  }, [locale]);
}
