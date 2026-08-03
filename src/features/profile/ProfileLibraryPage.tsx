export interface ProfileDoc {
  id: string;
  title: string;
  originalPath: string;
  importedAtMs: number;
  enabled: boolean;
}

export const MAX_PROFILES = 10;

export interface ProfileLibraryPageProps {
  profiles: ProfileDoc[];
  onImport?: (files: FileList) => void;
  onToggle?: (id: string, enabled: boolean) => void;
  onRemove?: (id: string) => void;
}

/** 资料库页面：展示已导入文档，支持导入/启用/移除（最多 10 份）。 */
export function ProfileLibraryPage({
  profiles,
  onImport,
  onToggle,
  onRemove,
}: ProfileLibraryPageProps) {
  const atLimit = profiles.length >= MAX_PROFILES;
  return (
    <div className="profile-library" data-testid="profile-library">
      <header className="profile-library__header">
        <h2>资料库</h2>
        <span className="profile-library__count" aria-label="资料数量">
          {profiles.length}/{MAX_PROFILES}
        </span>
      </header>

      <label className="profile-library__import">
        <input
          type="file"
          accept=".pdf,.docx,.txt,.md,.markdown"
          multiple
          disabled={atLimit}
          onChange={(e) => {
            if (e.target.files && e.target.files.length > 0) {
              onImport?.(e.target.files);
            }
            e.target.value = "";
          }}
        />
        <span data-testid="import-button">
          {atLimit ? "已达上限（10 份）" : "导入资料"}
        </span>
      </label>

      {profiles.length === 0 ? (
        <p className="profile-library__empty">
          尚未导入资料。支持 PDF / DOCX / TXT / Markdown，最多 {MAX_PROFILES} 份，仅在本机解析。
        </p>
      ) : (
        <ul className="profile-library__list">
          {profiles.map((p) => (
            <li key={p.id} className="profile-library__item" data-enabled={p.enabled}>
              <div className="profile-library__item-main">
                <span className="profile-library__title">{p.title}</span>
                <span className="profile-library__path" title={p.originalPath}>
                  {p.originalPath}
                </span>
              </div>
              <button
                type="button"
                className="profile-library__toggle"
                aria-pressed={p.enabled}
                onClick={() => onToggle?.(p.id, !p.enabled)}
              >
                {p.enabled ? "启用中" : "已停用"}
              </button>
              <button
                type="button"
                className="profile-library__remove"
                aria-label={`移除 ${p.title}`}
                onClick={() => onRemove?.(p.id)}
              >
                移除
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
