import { useCallback, useEffect, useState } from "react";
import { clearAllData, getSettings, saveSettings, testProviderConnection } from "../../lib/tauri";
import type { AppSettings } from "../../types/domain";

const DEFAULT_BASE_URLS: Record<string, string> = {
  deepseek: "https://api.deepseek.com",
  openai: "https://api.openai.com/v1",
  custom: "http://127.0.0.1:11434/v1",
};

export interface SettingsPageProps {
  initial?: AppSettings;
  onSaved?: (settings: AppSettings) => void;
}

/** 设置页：provider / base URL / model / API Key（永不回显）/ 连接测试 / 保留天数 / 清除数据。 */
export function SettingsPage({ initial, onSaved }: SettingsPageProps) {
  const [settings, setSettings] = useState<AppSettings | null>(initial ?? null);
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [testResult, setTestResult] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (initial) return;
    void getSettings().then(setSettings);
  }, [initial]);

  const update = useCallback((patch: Partial<AppSettings>) => {
    setSettings((s) => (s ? { ...s, ...patch } : s));
  }, []);

  const changeProvider = useCallback(
    (kind: string) => {
      update({
        providerKind: kind as AppSettings["providerKind"],
        baseUrl: DEFAULT_BASE_URLS[kind] ?? "",
      });
    },
    [update],
  );

  if (!settings) {
    return <p className="settings-page" data-testid="settings-loading">正在加载设置…</p>;
  }

  const handleSave = async () => {
    setSaving(true);
    try {
      await saveSettings(settings, apiKeyInput || null);
      setApiKeyInput("");
      onSaved?.(settings);
    } finally {
      setSaving(false);
    }
  };

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      setTestResult(await testProviderConnection(settings, apiKeyInput || null));
    } catch (e) {
      setTestResult(String(e));
    } finally {
      setTesting(false);
    }
  };

  const handleClearAll = async () => {
    if (!window.confirm("确定清除全部会议记录、转写与资料？此操作不可撤销。")) return;
    await clearAllData();
    setTestResult("已清除全部数据");
  };

  return (
    <div className="settings-page" data-testid="settings-page">
      <h2>设置</h2>

      <label className="settings-page__field">
        <span>答案服务</span>
        <select
          value={settings.providerKind}
          onChange={(e) => changeProvider(e.target.value)}
        >
          <option value="deepseek">DeepSeek</option>
          <option value="openai">OpenAI</option>
          <option value="custom">自定义 OpenAI 兼容（本机 Ollama / LM Studio）</option>
        </select>
      </label>

      <label className="settings-page__field">
        <span>Base URL</span>
        <input
          type="url"
          value={settings.baseUrl}
          onChange={(e) => update({ baseUrl: e.target.value })}
        />
      </label>

      <label className="settings-page__field">
        <span>模型 ID</span>
        <input
          type="text"
          value={settings.model}
          onChange={(e) => update({ model: e.target.value })}
        />
      </label>

      <label className="settings-page__field">
        <span>API Key（保存于 Windows 凭据管理器，永不回显）</span>
        <input
          type="password"
          value={apiKeyInput}
          placeholder={settings.hasApiKey ? "已保存，输入新值可覆盖" : "未设置"}
          onChange={(e) => setApiKeyInput(e.target.value)}
          autoComplete="off"
        />
      </label>

      <div className="settings-page__row">
        <button type="button" onClick={() => void handleTest()} disabled={testing}>
          {testing ? "测试中…" : "连接测试"}
        </button>
        <button type="button" onClick={() => void handleSave()} disabled={saving}>
          {saving ? "保存中…" : "保存设置"}
        </button>
      </div>
      {testResult && (
        <p className="settings-page__result" data-testid="test-result">
          {testResult}
        </p>
      )}

      <label className="settings-page__field">
        <span>保留天数（默认 7 天自动删除未固定的会议记录）</span>
        <input
          type="number"
          min={1}
          max={365}
          value={settings.retentionDays}
          onChange={(e) => update({ retentionDays: Number(e.target.value) || 7 })}
        />
      </label>

      <label className="settings-page__field settings-page__check">
        <input
          type="checkbox"
          checked={settings.microphoneEnabled}
          onChange={(e) => update({ microphoneEnabled: e.target.checked })}
        />
        <span>启用麦克风转写（独立于系统音频）</span>
      </label>

      <hr />

      <button
        type="button"
        className="settings-page__danger"
        onClick={() => void handleClearAll()}
      >
        清除全部数据
      </button>
    </div>
  );
}
