import type { ReactNode } from "react";
import ReactMarkdown, { defaultUrlTransform } from "react-markdown";
import remarkGfm from "remark-gfm";
import { sourceFromLink, type VerifiableSource } from "./SourceInspector";

const ARTIFACT_PROTOCOL = "galen-artifact://";

interface ArtifactMarkdownProps {
  children: string;
  onOpenArtifact?: (artifactId: string) => void;
  onOpenSource?: (source: VerifiableSource) => void;
}

export function artifactHref(artifactId: string): string {
  return `${ARTIFACT_PROTOCOL}${encodeURIComponent(artifactId)}`;
}

// Model output and imported notes often contain conventional identifiers as
// plain text. Normalize only explicit labels, so ordinary numbers and prose
// remain untouched while every declared source becomes directly verifiable.
export function linkifyEvidenceIdentifiers(markdown: string): string {
  return markdown
    .replace(
      /\bPMID\s*[:：]?\s*(\d{5,9})\b/gi,
      (_match, pmid: string) => `[PMID: ${pmid}](https://pubmed.ncbi.nlm.nih.gov/${pmid}/)`,
    )
    .replace(
      /\bDOI\s*[:：]?\s*(10\.\d{4,9}\/[A-Za-z0-9._;()/:+-]+)(?!\])/gi,
      (_match, doi: string) => `[DOI: ${doi}](https://doi.org/${doi})`,
    );
}

export function ArtifactMarkdown({ children, onOpenArtifact, onOpenSource }: ArtifactMarkdownProps) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      urlTransform={(url) => url.startsWith(ARTIFACT_PROTOCOL) ? url : defaultUrlTransform(url)}
      components={{
        a: ({ href = "", children: label }: { href?: string; children?: ReactNode }) => {
          if (!href.startsWith(ARTIFACT_PROTOCOL)) {
            const source = sourceFromLink(href, extractLabel(label));
            return (
              <a
                href={href}
                target="_blank"
                rel="noreferrer"
                onClick={(event) => {
                  if (!source || !onOpenSource) return;
                  event.preventDefault();
                  onOpenSource(source);
                }}
              >
                {label}
              </a>
            );
          }
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
      {linkifyEvidenceIdentifiers(children)}
    </ReactMarkdown>
  );
}

function extractLabel(label: ReactNode): string {
  if (typeof label === "string" || typeof label === "number") return String(label);
  if (Array.isArray(label)) return label.map(extractLabel).join("");
  return "";
}
