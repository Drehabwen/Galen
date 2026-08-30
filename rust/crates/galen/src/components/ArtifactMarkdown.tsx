import type { ReactNode } from "react";
import ReactMarkdown, { defaultUrlTransform } from "react-markdown";
import remarkGfm from "remark-gfm";

const ARTIFACT_PROTOCOL = "galen-artifact://";

interface ArtifactMarkdownProps {
  children: string;
  onOpenArtifact?: (artifactId: string) => void;
}

export function artifactHref(artifactId: string): string {
  return `${ARTIFACT_PROTOCOL}${encodeURIComponent(artifactId)}`;
}

export function ArtifactMarkdown({ children, onOpenArtifact }: ArtifactMarkdownProps) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      urlTransform={(url) => url.startsWith(ARTIFACT_PROTOCOL) ? url : defaultUrlTransform(url)}
      components={{
        a: ({ href = "", children: label }: { href?: string; children?: ReactNode }) => {
          if (!href.startsWith(ARTIFACT_PROTOCOL)) return <a href={href}>{label}</a>;
          const artifactId = decodeURIComponent(href.slice(ARTIFACT_PROTOCOL.length));
          return (
            <a
              href={href}
              className="artifact-preview-link"
              onClick={(event) => {
                event.preventDefault();
                onOpenArtifact?.(artifactId);
              }}
            >
              {label}<span aria-hidden="true"> ↗</span>
            </a>
          );
        },
      }}
    >
      {children}
    </ReactMarkdown>
  );
}
