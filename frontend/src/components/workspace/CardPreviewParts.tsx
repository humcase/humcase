import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

/** An image inside a card's excerpt: named, never loaded. */
export function PreviewImage({ alt }: { alt?: string }) {
  const { t } = useTranslation();
  return <span className="workspace-note-card__preview-image">{t('workspace.cards.imagePreview', { alt: alt || t('workspace.cards.noDescription') })}</span>;
}

/** A link inside a card's excerpt: its text only, the card opens the case. */
export function PreviewLink({ children }: { children?: ReactNode }) {
  return <span className="workspace-note-card__preview-link">{children}</span>;
}
