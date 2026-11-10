import { FolderSearch, Gamepad2, Loader2, Plus, Settings2, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useSystem } from "../../context/SystemContext";
import { useToast } from "../../context/ToastContext";
import { useJob } from "../../hooks/useJob";
import { api, errorMessage, type Game, type GameProfile } from "../../lib/api";
import { formatBytes } from "../../lib/format";
import { Badge, Card, Skeleton } from "../ui/Card";
import { Switch } from "../ui/Controls";
import { Modal } from "../ui/Modal";

const SOURCE: Record<Game["source"], string> = { steam: "Steam", epic: "Epic Games", gog: "GOG", riot: "Riot", roblox: "Roblox", custom: "Added manually" };
const GPU_LABEL = { default: "Let Windows decide", powerSaving: "Power saving GPU", highPerformance: "High-performance GPU" };
const PRIORITY_LABEL = { normal: "Normal", aboveNormal: "Above normal", high: "High" };

function summary(p: GameProfile | null) {
  if (!p) return null;
  const parts = [];
  if (p.gpu !== "default") parts.push(GPU_LABEL[p.gpu]);
  if (!p.fullscreenOptimizations) parts.push("Fullscreen optimizations off");
  if (p.priority !== "normal") parts.push(`${PRIORITY_LABEL[p.priority]} priority`);
  return parts.length ? parts.join(" · ") : "Windows defaults";
}

export function GameProfiles() {
  const { appInfo } = useSystem();
  const toast = useToast();
  const { run, busy } = useJob();
  const [games, setGames] = useState<[Game, GameProfile | null][] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [edit, setEdit] = useState<{ game: Game; profile: GameProfile } | null>(null);
  const [pick, setPick] = useState<{ game: Game; options: { path: string; sizeBytes: number }[] | null } | null>(null);
  const [adding, setAdding] = useState<{ name: string; exe: string } | null>(null);

  const load = useCallback(async () => {
    try {
      setGames(await api.games());
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const openPicker = async (game: Game) => {
    setPick({ game, options: null });
    try {
      setPick({ game, options: await api.gameCandidates(game.id) });
    } catch (e) {
      toast("error", "Could not list executables", errorMessage(e));
      setPick(null);
    }
  };

  const choose = async (game: Game, exe: string) => {
    try {
      await api.chooseGameExe(game.id, exe);
      setPick(null);
      await load();
    } catch (e) {
      toast("error", "Could not use that file", errorMessage(e));
    }
  };

  const save = async () => {
    if (!edit?.game.exe) return;
    const { game, profile } = edit;
    setEdit(null);
    await run(game.id, () => api.setGameProfile(game.exe!, profile), { success: `Profile saved for ${game.name}. Restart the game to apply it.`, failure: `Could not save the ${game.name} profile` });
    await load();
  };

  const add = async () => {
    if (!adding) return;
    try {
      await api.addGame(adding.name, adding.exe.trim().replace(/^"|"$/g, ""));
      setAdding(null);
      await load();
    } catch (e) {
      toast("error", "Could not add the game", errorMessage(e));
    }
  };

  return (
    <Card
      title="Game profiles"
      icon={Gamepad2}
      actions={
        <button className="btn-ghost px-3 py-1.5 text-xs" onClick={() => setAdding({ name: "", exe: "" })}>
          <Plus size={13} /> Add a game
        </button>
      }
    >
      <p className="mb-3 text-xs text-muted">
        Per-game Windows settings, stored by Windows itself so they work even when Blob Optimizers is closed. Detected from Steam, Epic, GOG, Riot and Roblox.
      </p>
      {error && <p className="text-xs text-bad">{error}</p>}
      {!games ? (
        <Skeleton className="h-32 w-full" />
      ) : games.length === 0 ? (
        <p className="py-6 text-center text-sm text-muted">No games detected. Use "Add a game" to add one by its .exe path.</p>
      ) : (
        <div className="divide-y divide-line/40 overflow-hidden rounded-xl border border-line/60">
          {games.map(([g, p]) => (
            <div key={g.id} className="flex items-center gap-3 px-4 py-3">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="font-medium">{g.name}</span>
                  <Badge>{SOURCE[g.source]}</Badge>
                </div>
                <div className="selectable truncate font-mono text-[11px] text-muted" title={g.exe ?? g.installDir}>
                  {g.exe ?? "Choose the game's executable to create a profile"}
                </div>
                {p && <div className="mt-0.5 text-xs text-muted">{summary(p)}</div>}
              </div>
              {busy === g.id ? (
                <Loader2 size={16} className="animate-spin text-accent" />
              ) : (
                <>
                  {g.source === "custom" && (
                    <button className="rounded-lg p-1.5 text-muted hover:bg-raised hover:text-bad" title="Remove from list" onClick={() => api.removeGame(g.id).then(load)}>
                      <Trash2 size={14} />
                    </button>
                  )}
                  {g.source !== "custom" && (
                    <button className="btn-ghost px-2.5 py-1 text-xs" onClick={() => openPicker(g)}>
                      <FolderSearch size={13} /> {g.exe ? "Change exe" : "Choose exe"}
                    </button>
                  )}
                  <button className="btn-ghost px-2.5 py-1 text-xs" disabled={!g.exe || !p} onClick={() => p && setEdit({ game: g, profile: { ...p } })}>
                    <Settings2 size={13} /> Profile
                  </button>
                </>
              )}
            </div>
          ))}
        </div>
      )}

      <Modal
        open={!!edit}
        title={`${edit?.game.name} profile`}
        onClose={() => setEdit(null)}
        width={560}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setEdit(null)}>
              Cancel
            </button>
            <button className="btn-primary" onClick={save}>
              Save profile
            </button>
          </>
        }
      >
        {edit && (
          <div className="space-y-5 text-[13px]">
            <div>
              <div className="label mb-2">Graphics processor</div>
              {(Object.keys(GPU_LABEL) as GameProfile["gpu"][]).map((k) => (
                <label key={k} className="flex cursor-pointer items-center gap-2.5 py-1">
                  <input type="radio" checked={edit.profile.gpu === k} onChange={() => setEdit({ ...edit, profile: { ...edit.profile, gpu: k } })} className="accent-[rgb(var(--c-accent))]" />
                  {GPU_LABEL[k]}
                </label>
              ))}
              <p className="mt-1 text-xs text-muted">Same as Windows' Graphics settings. Matters on PCs with both integrated and dedicated graphics.</p>
            </div>
            <div className="flex items-start gap-3">
              <div className="flex-1">
                <div className="font-medium">Fullscreen optimizations</div>
                <p className="text-xs text-muted">
                  Windows runs fullscreen games through the desktop compositor for faster alt-tab and overlays. Turning it off forces classic exclusive
                  fullscreen, which helps some older DirectX 9-11 games with stutter or input lag. Leave on unless a game has problems.
                </p>
              </div>
              <Switch label="Fullscreen optimizations" checked={edit.profile.fullscreenOptimizations} onChange={(v) => setEdit({ ...edit, profile: { ...edit.profile, fullscreenOptimizations: v } })} />
            </div>
            <div>
              <div className="label mb-2">CPU priority when the game starts</div>
              <select
                value={edit.profile.priority}
                onChange={(e) => setEdit({ ...edit, profile: { ...edit.profile, priority: e.target.value as GameProfile["priority"] } })}
                className="rounded-lg border border-line bg-base/60 px-2 py-1.5 text-sm outline-none focus:border-accent/60"
              >
                {(Object.keys(PRIORITY_LABEL) as GameProfile["priority"][]).map((k) => (
                  <option key={k} value={k}>
                    {PRIORITY_LABEL[k]}
                  </option>
                ))}
              </select>
              <p className="mt-1 text-xs text-muted">
                Applies to every program with this file name. Can help when background apps compete for CPU; High can make the rest of Windows sluggish while
                the game runs. Some anti-cheat systems ignore it.{!appInfo?.elevated && " Changing it needs administrator permission."}
              </p>
            </div>
          </div>
        )}
      </Modal>

      <Modal open={!!pick} title={`Which file starts ${pick?.game.name}?`} onClose={() => setPick(null)} width={640}>
        {!pick?.options ? (
          <Skeleton className="h-24 w-full" />
        ) : pick.options.length === 0 ? (
          <p className="text-sm text-muted">No executables found in {pick.game.installDir}.</p>
        ) : (
          <ul className="space-y-1.5">
            {pick.options.map((o) => (
              <li key={o.path}>
                <button className="flex w-full items-center gap-3 rounded-xl bg-raised/60 px-3 py-2 text-left hover:bg-raised" onClick={() => choose(pick.game, o.path)}>
                  <span className="min-w-0 flex-1 truncate font-mono text-[12px]" title={o.path}>
                    {o.path.slice(pick.game.installDir.length) || o.path}
                  </span>
                  <span className="text-xs text-muted">{formatBytes(o.sizeBytes)}</span>
                </button>
              </li>
            ))}
            <li className="pt-1 text-xs text-muted">Most likely first. The game's main executable is usually the largest file or one ending in "-Shipping".</li>
          </ul>
        )}
      </Modal>

      <Modal
        open={!!adding}
        title="Add a game"
        onClose={() => setAdding(null)}
        footer={
          <>
            <button className="btn-ghost" onClick={() => setAdding(null)}>
              Cancel
            </button>
            <button className="btn-primary" disabled={!adding?.name.trim() || !adding?.exe.trim()} onClick={add}>
              Add
            </button>
          </>
        }
      >
        {adding && (
          <div className="space-y-3 text-[13px]">
            <label className="block">
              <span className="label">Name</span>
              <input value={adding.name} onChange={(e) => setAdding({ ...adding, name: e.target.value })} className="mt-1 w-full rounded-xl border border-line bg-base/60 px-3 py-2 outline-none focus:border-accent/60" />
            </label>
            <label className="block">
              <span className="label">Full path to the game's .exe</span>
              <input
                value={adding.exe}
                onChange={(e) => setAdding({ ...adding, exe: e.target.value })}
                placeholder="C:\Games\MyGame\MyGame.exe"
                className="mt-1 w-full rounded-xl border border-line bg-base/60 px-3 py-2 font-mono text-[12px] outline-none focus:border-accent/60"
              />
            </label>
            <p className="text-xs text-muted">Tip: in File Explorer, Shift + right-click the .exe and choose "Copy as path".</p>
          </div>
        )}
      </Modal>
    </Card>
  );
}
