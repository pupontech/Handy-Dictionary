import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import ReactMarkdown from "react-markdown";
import { Button } from "../../ui/Button";
import { Input } from "../../ui/Input";
import userGuide from "../../../../docs/GITHUB-SYNC.md?raw";

interface SyncStatus {
  clientId: string;
  appSlug: string;
  connected: boolean;
  repository: {
    owner: string;
    name: string;
    branch: string;
    id: number;
  } | null;
  lastSync: number | null;
  firstSync: boolean;
}
interface DeviceCode {
  userCode: string;
  verificationUri: string;
  interval: number;
}
interface Conflict {
  key: string;
  local: string | null;
  remote: string | null;
}
interface SyncResult {
  status: "synced" | "conflicts";
  conflicts?: Conflict[];
}

export const DictionarySync: React.FC<{ onSynced: () => Promise<void> }> = ({
  onSynced,
}) => {
  const { t } = useTranslation();
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [clientId, setClientId] = useState("");
  const [appSlug, setAppSlug] = useState("");
  const [owner, setOwner] = useState("");
  const [name, setName] = useState("");
  const [device, setDevice] = useState<DeviceCode | null>(null);
  const [conflicts, setConflicts] = useState<Conflict[]>([]);
  const [confirmed, setConfirmed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const mounted = useRef(false);
  const running = useRef(false);
  const key = (suffix: string) => `settings.dictionary.sync.${suffix}`;

  useEffect(() => {
    let active = true;
    mounted.current = true;
    invoke<SyncStatus>("dictionary_sync_status")
      .then((result) => {
        if (!active) return;
        setStatus(result);
        setClientId(result.clientId);
        setAppSlug(result.appSlug);
        setOwner(result.repository?.owner ?? "");
        setName(result.repository?.name ?? "");
      })
      .catch((reason: unknown) => {
        if (active) setError(String(reason));
      });
    return () => {
      active = false;
      mounted.current = false;
    };
  }, []);

  const refresh = async () => {
    const result = await invoke<SyncStatus>("dictionary_sync_status");
    if (mounted.current) setStatus(result);
  };
  const run = async (action: () => Promise<void>) => {
    if (running.current) return;
    running.current = true;
    setBusy(true);
    setError("");
    setMessage("");
    try {
      await action();
    } catch (reason) {
      if (mounted.current) setError(String(reason));
    } finally {
      running.current = false;
      if (mounted.current) setBusy(false);
    }
  };
  const open = (url: string) =>
    run(async () => {
      await openUrl(url);
    });
  const sync = (resolution: "local" | "remote" | null = null) =>
    run(async () => {
      const result = await invoke<SyncResult>("dictionary_sync_now", {
        resolution,
        confirmFirstSync: confirmed,
      });
      if (!mounted.current) return;
      if (result.status === "conflicts") {
        setConflicts(result.conflicts ?? []);
        return;
      }
      setConflicts([]);
      await onSynced();
      await refresh();
      if (mounted.current) setMessage("synced");
    });

  return (
    <section
      className="rounded-lg border border-mid-gray/20 p-4 space-y-4"
      aria-labelledby="dictionary-sync-title"
    >
      <h2 id="dictionary-sync-title" className="text-lg font-semibold">
        {t(key("title"))}
      </h2>
      <p className="text-sm text-text/70">{t(key("description"))}</p>
      <p className="text-sm">{t(key("privacy"))}</p>
      {!status?.clientId && (
        <p className="text-sm" role="note">
          {t(key("unconfigured"))}
        </p>
      )}
      <details>
        <summary className="cursor-pointer text-sm">
          {t(key("appConfiguration"))}
        </summary>
        <div className="mt-3 grid gap-3">
          <label className="text-sm">
            {t(key("clientId"))}
            <Input
              aria-label={t(key("clientId"))}
              value={clientId}
              disabled={busy || status?.connected}
              onChange={(e) => setClientId(e.target.value)}
            />
          </label>
          <label className="text-sm">
            {t(key("appSlug"))}
            <Input
              aria-label={t(key("appSlug"))}
              value={appSlug}
              disabled={busy || status?.connected}
              onChange={(e) => setAppSlug(e.target.value)}
            />
          </label>
          <p className="text-sm text-text/70">
            {t(key("publicConfiguration"))}
          </p>
        </div>
      </details>
      <div className="flex flex-wrap gap-2">
        <Button
          variant="secondary"
          disabled={busy}
          onClick={() => open("https://github.com/new")}
        >
          {t(key("createRepository"))}
        </Button>
        <Button
          variant="secondary"
          disabled={busy || !/^[a-zA-Z0-9-]+$/.test(appSlug)}
          onClick={() =>
            open(`https://github.com/apps/${appSlug}/installations/new`)
          }
        >
          {t(key("installApp"))}
        </Button>
        <Button
          disabled={busy || !clientId || !appSlug}
          onClick={() =>
            run(async () => {
              const code = await invoke<DeviceCode>("dictionary_sync_begin", {
                clientId,
                appSlug,
              });
              if (mounted.current) {
                setDevice(code);
                setConflicts([]);
              }
            })
          }
        >
          {t(key(status?.connected ? "reconnect" : "connect"))}
        </Button>
        {(status?.connected || device) && (
          <Button
            variant="secondary"
            disabled={busy}
            onClick={() =>
              run(async () => {
                await invoke("dictionary_sync_disconnect");
                if (mounted.current) {
                  setDevice(null);
                  setConflicts([]);
                }
                await refresh();
              })
            }
          >
            {t(key("disconnect"))}
          </Button>
        )}
      </div>
      {device && (
        <div className="rounded border border-mid-gray/20 p-3 space-y-2">
          <p>{t(key("authorizationInstructions"))}</p>
          <p
            className="font-mono text-xl select-all"
            aria-label={t(key("deviceCode"))}
          >
            {device.userCode}
          </p>
          <div className="flex flex-wrap gap-2">
            <Button
              variant="secondary"
              disabled={busy}
              onClick={() => open("https://github.com/login/device")}
            >
              {t(key("openAuthorization"))}
            </Button>
            <Button
              disabled={busy}
              onClick={() =>
                run(async () => {
                  const connected = await invoke<boolean>(
                    "dictionary_sync_poll",
                  );
                  if (!mounted.current) return;
                  if (connected) {
                    setDevice(null);
                    await refresh();
                    setMessage("connectedMessage");
                  } else {
                    setMessage("authorizationPending");
                  }
                })
              }
            >
              {t(key("checkAuthorization"))}
            </Button>
          </div>
        </div>
      )}
      {status?.connected && (
        <div className="space-y-3">
          <p className="text-sm">{t(key("connected"))}</p>
          <div className="grid sm:grid-cols-2 gap-3">
            <label className="text-sm">
              {t(key("owner"))}
              <Input
                aria-label={t(key("owner"))}
                value={owner}
                disabled={busy}
                onChange={(e) => setOwner(e.target.value)}
              />
            </label>
            <label className="text-sm">
              {t(key("repositoryName"))}
              <Input
                aria-label={t(key("repositoryName"))}
                value={name}
                disabled={busy}
                onChange={(e) => setName(e.target.value)}
              />
            </label>
          </div>
          <Button
            variant="secondary"
            disabled={busy || !owner || !name}
            onClick={() =>
              run(async () => {
                await invoke("dictionary_sync_select_repository", {
                  owner,
                  name,
                });
                if (mounted.current) {
                  setConflicts([]);
                  setConfirmed(false);
                }
                await refresh();
              })
            }
          >
            {t(key("selectRepository"))}
          </Button>
        </div>
      )}
      {status?.repository && (
        <p className="text-sm break-all">
          {t(key("selectedRepository"), {
            repository: `${status.repository.owner}/${status.repository.name}`,
            branch: status.repository.branch,
          })}
        </p>
      )}
      {status?.firstSync && (
        <label className="flex items-start gap-2 text-sm">
          <input
            type="checkbox"
            checked={confirmed}
            disabled={busy}
            onChange={(e) => setConfirmed(e.target.checked)}
          />
          <span>{t(key("firstSyncConsent"))}</span>
        </label>
      )}
      <Button
        disabled={
          busy ||
          !status?.connected ||
          !status.repository ||
          (status.firstSync && !confirmed)
        }
        onClick={() => sync()}
      >
        {t(key(busy ? "working" : "syncNow"))}
      </Button>
      {status?.lastSync && (
        <p className="text-sm text-text/70">
          {t(key("lastSync"), {
            time: new Date(status.lastSync * 1000).toLocaleString(),
          })}
        </p>
      )}
      {conflicts.length > 0 && (
        <div
          className="space-y-3"
          role="region"
          aria-label={t(key("conflicts"))}
        >
          <h3 className="font-semibold">{t(key("conflicts"))}</h3>
          <p className="text-sm">{t(key("conflictExplanation"))}</p>
          <ul className="max-h-64 overflow-auto space-y-3 text-sm">
            {conflicts.map((conflict) => (
              <li key={conflict.key} className="break-words">
                <strong>{conflict.key}</strong>
                <p>
                  {t(key("localValue"), {
                    value: conflict.local ?? t(key("deleted")),
                  })}
                </p>
                <p>
                  {t(key("remoteValue"), {
                    value: conflict.remote ?? t(key("deleted")),
                  })}
                </p>
              </li>
            ))}
          </ul>
          <div className="flex flex-wrap gap-2">
            <Button disabled={busy} onClick={() => sync("local")}>
              {t(key("keepLocal"))}
            </Button>
            <Button
              variant="secondary"
              disabled={busy}
              onClick={() => sync("remote")}
            >
              {t(key("keepRemote"))}
            </Button>
            <Button
              variant="secondary"
              disabled={busy}
              onClick={() => setConflicts([])}
            >
              {t(key("cancel"))}
            </Button>
          </div>
        </div>
      )}
      {error && (
        <p role="alert" className="text-sm text-red-500">
          {t(key(`errors.${error}`), {
            defaultValue: t(key("errors.unknown")),
          })}
        </p>
      )}
      {message && (
        <p role="status" className="text-sm">
          {t(key(message))}
        </p>
      )}
      <details>
        <summary className="cursor-pointer font-medium">
          {t(key("guide"))}
        </summary>
        <div className="mt-3 max-h-[36rem] overflow-auto space-y-3 text-sm [&_h1]:text-xl [&_h2]:text-lg [&_h2]:font-semibold [&_h3]:font-semibold [&_ul]:list-disc [&_ul]:pl-5 [&_ol]:list-decimal [&_ol]:pl-5 [&_p]:my-2 [&_a]:underline [&_pre]:overflow-auto">
          <ReactMarkdown
            components={{
              a: ({ href, children }) => (
                <a
                  href={href}
                  onClick={(event) => {
                    event.preventDefault();
                    if (href?.startsWith("https://")) open(href);
                  }}
                >
                  {children}
                </a>
              ),
            }}
          >
            {userGuide}
          </ReactMarkdown>
        </div>
      </details>
    </section>
  );
};
