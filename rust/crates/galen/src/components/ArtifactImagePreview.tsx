import { useEffect, useState } from "react";

function useObjectUrl(blob: Blob): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    const objectUrl = URL.createObjectURL(blob);
    setUrl(objectUrl);
    return () => URL.revokeObjectURL(objectUrl);
  }, [blob]);
  return url;
}

export function ImageView({ blob, path }: { blob: Blob; path: string }) {
  const url = useObjectUrl(blob);
  if (!url) return <div className="artifact-empty">正在准备图片预览…</div>;
  return (
    <div className="artifact-preview-scroll">
      <img className="artifact-image" src={url} alt={path.split(/[\\/]/).pop() ?? "产物图片"} data-testid="artifact-image-view" />
    </div>
  );
}
