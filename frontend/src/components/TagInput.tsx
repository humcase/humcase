import React, { useState, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import './ui/metadata.css';
import { X } from '@phosphor-icons/react';
import { Badge } from './ui/Badge';

interface TagInputProps {
  tags: string[];
  onChange: (tags: string[]) => void;
  availableTags: string[];
}

export default function TagInput({ tags, onChange, availableTags }: TagInputProps) {
  const { t } = useTranslation();
  const [input, setInput] = useState('');
  const [showSuggestions, setShowSuggestions] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  // Defensive guard: a non-string tag must never crash the render (`toLowerCase`).
  const filteredTags = availableTags.filter(
    fileName => typeof fileName === 'string' && fileName.toLowerCase().includes(input.toLowerCase()) && !tags.includes(fileName)
  );

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setShowSuggestions(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' || e.key === ',') {
      e.preventDefault();
      addTag(input);
    } else if (e.key === 'Backspace' && input === '' && tags.length > 0) {
      removeTag(tags[tags.length - 1]);
    }
  };

  const addTag = (tag: string) => {
    // Transliteration of accents ("café" → "cafe"): the old filter silently
    // removed accented characters ("café" → "caf").
    const cleanTag = tag
      .normalize('NFD')
      .replace(/[\u0300-\u036f]/g, '')
      .replace(/[^a-zA-Z0-9_-]/g, '')
      .trim();
    if (cleanTag && !tags.includes(cleanTag)) {
      onChange([...tags, cleanTag]);
    }
    setInput('');
    setShowSuggestions(false);
  };

  const removeTag = (tagToRemove: string) => {
    onChange(tags.filter(fileName => fileName !== tagToRemove));
  };

  return (
    <div className="ui-tag-field" ref={containerRef}>
      {tags.map(tag => (
        <Badge key={tag} variant="outline" className="ui-tag ui-tag--removable">
          #{tag}
          <button
            type="button"
            onClick={() => removeTag(tag)}
            aria-label={t('editor.tags.remove', { tag })}
            className="ui-tag-remove"
          >
            <X size={12} aria-hidden="true" />
          </button>
        </Badge>
      ))}
      <input
        aria-label={t('editor.tags.add')}
        type="text"
        value={input}
        onChange={(e) => {
          setInput(e.target.value);
          setShowSuggestions(true);
        }}
        onKeyDown={handleKeyDown}
        onFocus={() => setShowSuggestions(true)}
        placeholder={tags.length === 0 ? t('editor.tags.placeholder') : ""}
        className="ui-tag-field__input"
      />
      {showSuggestions && input && filteredTags.length > 0 && (
        <ul className="ui-menu ui-tag-field__suggestions">
          {filteredTags.map(tag => (
            <li key={tag}>
              <button type="button"
                onMouseDown={e => e.preventDefault()}
                onClick={() => addTag(tag)}
                className="ui-menu__item tag-suggestion">
                #{tag}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

