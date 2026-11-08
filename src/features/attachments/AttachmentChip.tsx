import { useState } from 'react';
import type { ReactElement } from 'react';
import { formatBytes } from '@/features/attachments/formatBytes';
import { useAssetUrl } from '@/features/remote/assetUrl';
import type { Attachment } from '@/lib/bindings/Attachment';
import './AttachmentChip.css';

interface AttachmentChipProps {
  attachment: Attachment;
  /** `input`: Chip über dem Eingabefeld (mit ×); `sent`: Chip in einer gesendeten Nachricht. */
  size: 'input' | 'sent';
  /** Die Session der Nachricht; ein gesendeter Anhang einer Session eines Hosts liegt dort. */
  sessionId?: string | undefined;
  onRemove?: (() => void) | undefined;
  /** Öffnet die Bildansicht; nur bei Bild-Anhängen wirksam. */
  onOpen?: (() => void) | undefined;
}

interface Dimensions {
  width: number;
  height: number;
}

export function AttachmentChip({
  attachment,
  size,
  sessionId,
  onRemove,
  onOpen,
}: AttachmentChipProps): ReactElement {
  const thumbSrc: string = useAssetUrl(attachment.path, sessionId ?? '', size === 'sent');
  // Die Abmessungen kennt nur das geladene Bild; bis dahin steht die Größe da.
  const [dimensions, setDimensions] = useState<Dimensions | null>(null);
  const isImage: boolean = attachment.kind === 'image';
  const meta: string =
    isImage && dimensions !== null
      ? `${String(dimensions.width)}×${String(dimensions.height)}`
      : formatBytes(attachment.sizeBytes);

  function renderLeading(): ReactElement {
    if (!isImage) {
      return <FileIcon />;
    }
    const thumb: ReactElement = (
      <img
        className="attachment-chip__thumb"
        src={thumbSrc}
        alt=""
        onLoad={(event): void => {
          setDimensions({
            width: event.currentTarget.naturalWidth,
            height: event.currentTarget.naturalHeight,
          });
        }}
      />
    );
    if (onOpen === undefined) {
      return thumb;
    }
    return (
      <button
        type="button"
        className="attachment-chip__open"
        title="Bild ansehen und kommentieren"
        aria-label={`${attachment.name} ansehen und kommentieren`}
        onClick={onOpen}
      >
        {thumb}
      </button>
    );
  }

  return (
    <span className={`attachment-chip attachment-chip--${size}`}>
      {renderLeading()}
      <span className="attachment-chip__name" title={attachment.name}>
        {attachment.name}
      </span>
      <span className="attachment-chip__meta">{meta}</span>
      {onRemove !== undefined && (
        <button
          type="button"
          className="attachment-chip__remove"
          aria-label={`${attachment.name} entfernen`}
          onClick={onRemove}
        >
          <svg
            width="10"
            height="10"
            viewBox="0 0 14 14"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.8"
            strokeLinecap="round"
            aria-hidden="true"
          >
            <path d="M3.5 3.5l7 7M10.5 3.5l-7 7" />
          </svg>
        </button>
      )}
    </span>
  );
}

function FileIcon(): ReactElement {
  return (
    <svg
      className="attachment-chip__icon"
      width="14"
      height="14"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M3.5 1.5h4.5l3 3v8h-7.5Z" />
      <path d="M8 1.5v3h3" />
    </svg>
  );
}
