import { useDeferredValue, useEffect, useMemo, useState } from "react";
import { LayoutGrid, RefreshCw, Search, SearchX, WifiOff } from "lucide-react";
import { AsyncButton } from "@/components/ui/AsyncButton";
import { Button } from "@/components/ui/Button";
import { EmptyState } from "@/components/ui/Feedback";
import { IconButton } from "@/components/ui/IconButton";
import { Input } from "@/components/ui/Input";
import { refreshApps } from "@/engine/actions";
import { refreshIcons } from "@/engine/sync";
import { fuzzyScore } from "@/lib/format";
import { useApps, useConnectedPhone, useIcons } from "@/store/engine";
import { PanelHeader } from "../PanelHeader";
import { AppTile } from "./AppTile";

export function AppsPanel() {
  const apps = useApps();
  const icons = useIcons();
  const connected = Boolean(useConnectedPhone());
  const [query, setQuery] = useState("");
  const [refreshing, setRefreshing] = useState(false);
  const deferred = useDeferredValue(query);

  useEffect(() => {
    void refreshIcons();
  }, []);

  const sorted = useMemo(
    () => [...apps].sort((a, b) => (a.name || a.pkg).localeCompare(b.name || b.pkg, undefined, { sensitivity: "base" })),
    [apps],
  );
  const shown = useMemo(() => {
    if (!deferred.trim()) return sorted;
    return sorted
      .map((app) => ({ app, score: fuzzyScore(deferred, `${app.name} ${app.pkg}`) }))
      .filter((item) => item.score >= 0)
      .sort((a, b) => b.score - a.score)
      .map((item) => item.app);
  }, [sorted, deferred]);

  const refresh = async () => {
    setRefreshing(true);
    await refreshApps();
    setRefreshing(false);
  };

  return (
    <>
      <PanelHeader title="Apps" subtitle={apps.length ? `${apps.length} installed · opens in a new window` : "Apps on your phone"}>
        <Input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onClear={() => setQuery("")}
          placeholder="Search apps"
          aria-label="Search apps"
          leading={<Search size={13} />}
          wrapperClassName="w-52"
          disabled={!apps.length}
          onKeyDown={(event) => {
            if (event.key === "Enter" && shown[0]) document.querySelector<HTMLButtonElement>("#app-grid button")?.click();
          }}
        />
        <IconButton
          label="Refresh app list"
          icon={<RefreshCw size={14} className={refreshing ? "animate-spin" : undefined} />}
          variant="solid"
          disabled={!connected || refreshing}
          onClick={refresh}
        />
      </PanelHeader>

      <div className="min-h-0 flex-1 overflow-y-auto px-4 pb-6">
        {!apps.length ? (
          connected ? (
            <EmptyState icon={<LayoutGrid size={20} />} title="Waiting for the app list" description="The phone sends its apps shortly after it connects.">
              <AsyncButton variant="primary" icon={<RefreshCw size={13} />} onClick={refreshApps}>
                Request app list
              </AsyncButton>
            </EmptyState>
          ) : (
            <EmptyState icon={<WifiOff size={20} />} title="Phone is offline" description="Connect your phone to browse and open its apps." />
          )
        ) : !shown.length ? (
          <EmptyState icon={<SearchX size={20} />} title="No apps found" description={`Nothing matches “${query}”.`}>
            <Button size="sm" onClick={() => setQuery("")}>
              Clear search
            </Button>
          </EmptyState>
        ) : (
          <ul id="app-grid" className="grid grid-cols-[repeat(auto-fill,minmax(92px,1fr))] gap-1">
            {shown.map((app, index) => (
              <AppTile key={`${app.pkg}:${app.userId}`} app={app} index={index} icon={(app.iconName && icons[app.iconName]) || ""} />
            ))}
          </ul>
        )}
      </div>
    </>
  );
}
