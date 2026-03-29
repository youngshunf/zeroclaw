import { useState, useMemo } from 'react';
import { Search } from 'lucide-react';
import { CONFIG_SECTIONS } from './configSections';
import { useTranslatedConfigSections } from './useTranslatedConfigSections';
import { useLocaleContext } from '@/App';
import { t_config } from './configTranslations';
import ConfigSection from './ConfigSection';
import type { FieldDef } from './types';

const CATEGORY_ORDER = [
  { key: 'all', label: 'All' },
  { key: 'general', label: 'General' },
  { key: 'security', label: 'Security' },
  { key: 'channels', label: 'Channels' },
  { key: 'runtime', label: 'Runtime' },
  { key: 'tools', label: 'Tools' },
  { key: 'memory', label: 'Memory' },
  { key: 'network', label: 'Network' },
  { key: 'advanced', label: 'Advanced' },
] as const;

interface Props {
  getFieldValue: (sectionPath: string, fieldKey: string) => unknown;
  setFieldValue: (sectionPath: string, fieldKey: string, value: unknown) => void;
  isFieldMasked: (sectionPath: string, fieldKey: string) => boolean;
}

export default function ConfigFormEditor({
  getFieldValue,
  setFieldValue,
  isFieldMasked,
}: Props) {
  const [search, setSearch] = useState('');
  const [activeCategory, setActiveCategory] = useState('all');
  const { locale } = useLocaleContext();
  const translatedSections = useTranslatedConfigSections();

  const isSearching = search.trim().length > 0;

  const filteredSections = useMemo(() => {
    if (isSearching) {
      const q = search.toLowerCase();
      return translatedSections.map((section) => {
        const titleMatch = section.title.toLowerCase().includes(q);
        const descMatch = section.description?.toLowerCase().includes(q);

        if (titleMatch || descMatch) {
          return { section, fields: undefined };
        }

        const matchingFields = section.fields.filter(
          (f: FieldDef) =>
            f.label.toLowerCase().includes(q) ||
            f.key.toLowerCase().includes(q) ||
            f.description?.toLowerCase().includes(q),
        );

        if (matchingFields.length > 0) {
          return { section, fields: matchingFields };
        }

        return null;
      }).filter(Boolean) as { section: (typeof CONFIG_SECTIONS)[0]; fields: FieldDef[] | undefined }[];
    }

    // Category filter
    const sections = activeCategory === 'all'
      ? translatedSections
      : translatedSections.filter((s) => s.category === activeCategory);

    return sections.map((s) => ({ section: s, fields: undefined }));
  }, [search, isSearching, activeCategory, translatedSections]);

  return (
    <div className="space-y-3">
      {/* Search */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-gray-500" />
        <input
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder={t_config('Search config fields...', locale)}
          className="w-full bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg pl-9 pr-3 py-2 text-sm focus:outline-none transition-colors"
        />
      </div>

      {/* Category pills — hidden during search */}
      {!isSearching && (
        <div className="flex flex-wrap gap-2">
          {CATEGORY_ORDER.map(({ key, label }) => (
            <button
              key={key}
              onClick={() => setActiveCategory(key)}
              className={`px-3 py-1 rounded-lg text-sm font-medium transition-colors ${
                activeCategory === key
                  ? 'bg-[var(--hx-purple)] text-white border border-[var(--hx-purple)]'
                  : 'bg-white dark:bg-gray-900 text-gray-600 dark:text-gray-400 border border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800 hover:text-gray-900 dark:hover:text-gray-200'
              }`}
            >
              {t_config(label, locale)}
            </button>
          ))}
        </div>
      )}

      {/* Sections */}
      {filteredSections.length === 0 ? (
        <div className="text-center py-12 text-gray-500 text-sm">
          {t_config('No matching config fields found.', locale)}
        </div>
      ) : (
        filteredSections.map(({ section, fields }) => (
          <ConfigSection
            key={section.path || '_root'}
            section={fields ? { ...section, defaultCollapsed: false } : section}
            getFieldValue={getFieldValue}
            setFieldValue={setFieldValue}
            isFieldMasked={isFieldMasked}
            visibleFields={fields}
          />
        ))
      )}
    </div>
  );
}
