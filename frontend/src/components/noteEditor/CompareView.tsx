import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { transformerUrlMarkdown } from '../../utils/imageRefs';
import { Card } from '../ui/Card';

/** The raw dictation beside the case as written now, to check what the AI kept. */
export function CompareView({ raw, content }: { raw: string | null | undefined; content: string }) {
  const { t } = useTranslation();
  return (
    <div className="flex-1 flex gap-4 overflow-hidden">
      <Card padding="none" className="note-editor-compare">
        <div className="note-editor-compare__heading">{t('editor.compare.raw')}</div>
        <div className="note-editor-compare__source">
          {raw || t('editor.compare.noSource')}
        </div>
      </Card>
      <Card padding="none" className="note-editor-compare">
        <div className="note-editor-compare__heading">{t('editor.compare.restructured')}</div>
        <div className="note-editor-compare__result prose prose-sm max-w-none prose-img:rounded-md">
          <ReactMarkdown remarkPlugins={[remarkGfm]} urlTransform={transformerUrlMarkdown}>{content || t('editor.compare.noContent')}</ReactMarkdown>
        </div>
      </Card>
    </div>
  );
}
