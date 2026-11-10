import { useCallback, useEffect, useState } from "react";
import { api, errorMessage, type InstalledApp } from "../lib/api";

/** Installed apps; the backend caches the list, so remounting is cheap until `refresh` is called. */
export function useApps() {
  const [apps, setApps] = useState<InstalledApp[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async (refresh: boolean) => {
    setLoading(true);
    setError(null);
    try {
      setApps(await api.apps(refresh));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load(false);
  }, [load]);

  return { apps, loading, error, refresh: () => load(true) };
}
