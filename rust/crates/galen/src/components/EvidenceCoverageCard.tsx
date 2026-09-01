import { useState } from "react";
import type {
  LiteratureCoverage,
  LiteratureProviderCoverage,
} from "../hooks/useLiteratureCoverage";

export type { LiteratureCoverage } from "../hooks/useLiteratureCoverage";

interface EvidenceCoverageCardProps {
  coverage: LiteratureCoverage | null;
  loading?: boolean;
  error?: string | null;
}

export function EvidenceCoverageCard({
  coverage,
  loading = false,
  error = null,
}: EvidenceCoverageCardProps) {
  const [expandedProviderId, setExpandedProviderId] = useState<string | null>(null);

  return (
    <section className="evidence-coverage-card" aria-labelledby="evidence-coverage-heading">
      <div className="evidence-coverage-heading-row">
        <h3 id="evidence-coverage-heading">文献来源覆盖</h3>
        {coverage?.taskId && <span className="evidence-coverage-task">活动任务</span>}
      </div>

      {loading && <p className="evidence-coverage-empty">正在加载覆盖范围…</p>}
      {!loading && error && <p className="evidence-coverage-error">{error}</p>}
      {!loading && !error && !coverage && (
        <p className="evidence-coverage-empty">暂无文献来源覆盖记录。</p>
      )}
      {!loading && coverage && (
        <>
          {coverage.hasLimitations && coverage.limitation && (
            <p className="evidence-coverage-limitation">{coverage.limitation}</p>
          )}
          <ul className="evidence-coverage-list">
            {coverage.providers.map((provider) => {
              const expanded = expandedProviderId === provider.providerId;
              return (
                <li key={provider.providerId} className="evidence-coverage-provider">
                  <button
                    type="button"
                    className="evidence-coverage-provider-summary"
                    aria-expanded={expanded}
                    onClick={() => setExpandedProviderId(expanded ? null : provider.providerId)}
                  >
                    <span>{provider.displayName}</span>
                    <span className={`evidence-coverage-state state-${provider.state}`}>
                      {providerStateLabel(provider)}
                    </span>
                  </button>
                  {expanded && (
                    <div className="evidence-coverage-details">
                      {provider.latestFinishedAt && (
                        <time dateTime={provider.latestFinishedAt}>{provider.latestFinishedAt}</time>
                      )}
                      {provider.latestQuery && (
                        <p><span>检索式</span>{provider.latestQuery}</p>
                      )}
                      {provider.state === "failed" && provider.errorClass && (
                        <p><span>错误类型</span>{provider.errorClass}</p>
                      )}
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
        </>
      )}
    </section>
  );
}

function providerStateLabel(provider: LiteratureProviderCoverage): string {
  switch (provider.state) {
    case "searched":
      return `已检索 · ${provider.resultCount ?? 0} 条`;
    case "failed":
      return provider.providerId === "cnki"
        ? "失败 · 不代表没有中文证据"
        : `失败${provider.errorClass ? ` · ${provider.errorClass}` : ""}`;
    case "connected_not_searched":
      return "尚未检索";
    case "configured_disabled":
      return "已禁用";
    case "unavailable":
      return provider.providerId === "cnki"
        ? "不可用 · 不代表没有中文证据"
        : "不可用";
    case "not_configured":
      return "未配置";
  }
}
