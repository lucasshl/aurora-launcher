import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ConfigFile,
  ContentKind,
  Cosmetic,
  CosmeticKind,
  CosmeticsModStatus,
  GalleryPage,
  CuratedStatus,
  GraphicsPreset,
  InstallReport,
  InstalledContent,
  ModrinthVersion,
  MojangCape,
  SearchResult,
  SkinModel,
  SortIndex,
  UpdateInfo,
  Wardrobe,
  Account,
  AccountsView,
  DeviceCode,
  ExitInfo,
  GameVersion,
  InstanceConfig,
  LaunchStarted,
  LogLine,
  NewInstance,
  ProgressEvent,
  Settings,
  SystemInfo,
} from "./types";

/** Erros do core chegam como string; normaliza qualquer formato. */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return JSON.stringify(e);
}

export const api = {
  // configurações
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  systemInfo: () => invoke<SystemInfo>("system_info"),

  // contas
  listAccounts: () => invoke<AccountsView>("list_accounts"),
  addOfflineAccount: (username: string) => invoke<Account>("add_offline_account", { username }),
  beginMicrosoftLogin: () => invoke<DeviceCode>("begin_microsoft_login"),
  completeMicrosoftLogin: (code: DeviceCode) => invoke<Account>("complete_microsoft_login", { code }),
  cancelMicrosoftLogin: (deviceCode: string) => invoke<void>("cancel_microsoft_login", { deviceCode }),
  removeAccount: (id: string) => invoke<AccountsView>("remove_account", { id }),
  setActiveAccount: (id: string) => invoke<AccountsView>("set_active_account", { id }),

  // versões
  /** kinds vazio = todas (release, snapshot, old_beta, old_alpha). */
  listMinecraftVersions: (kinds: string[]) => invoke<GameVersion[]>("list_minecraft_versions", { kinds }),
  listFabricVersions: (minecraftVersion: string) => invoke<string[]>("list_fabric_versions", { minecraftVersion }),

  // instâncias
  listInstances: () => invoke<InstanceConfig[]>("list_instances"),
  createInstance: (request: NewInstance) => invoke<InstanceConfig>("create_instance", { request }),
  updateInstance: (config: InstanceConfig) => invoke<InstanceConfig>("update_instance", { config }),
  deleteInstance: (id: string) => invoke<void>("delete_instance", { id }),
  openInstanceFolder: (id: string) => invoke<void>("open_instance_folder", { id }),
  setOfflineSkin: (instanceId: string, username: string, pngPath: string) =>
    invoke<void>("set_offline_skin", { instanceId, username, pngPath }),

  // execução
  launch: (instanceId: string, accountId: string) => invoke<LaunchStarted>("launch_instance", { instanceId, accountId }),
  kill: (instanceId: string) => invoke<boolean>("kill_instance", { instanceId }),
  runningInstances: () => invoke<string[]>("running_instances"),

  // mods, resource packs e shaders
  listContent: (instanceId: string, kind: ContentKind) => invoke<InstalledContent[]>("list_content", { instanceId, kind }),
  toggleContent: (instanceId: string, kind: ContentKind, fileName: string, enabled: boolean) =>
    invoke<void>("toggle_content", { instanceId, kind, fileName, enabled }),
  removeContent: (instanceId: string, kind: ContentKind, fileName: string) =>
    invoke<void>("remove_content", { instanceId, kind, fileName }),
  searchContent: (instanceId: string, kind: ContentKind, query: string, sort: SortIndex, offset: number) =>
    invoke<SearchResult>("search_content", { instanceId, kind, query, sort, offset }),
  contentVersions: (instanceId: string, kind: ContentKind, project: string) =>
    invoke<ModrinthVersion[]>("content_versions", { instanceId, kind, project }),
  installContent: (instanceId: string, kind: ContentKind, projects: string[], versionId?: string | null) =>
    invoke<InstallReport>("install_content", { instanceId, kind, projects, versionId: versionId ?? null }),
  checkContentUpdates: (instanceId: string, kind: ContentKind) =>
    invoke<UpdateInfo[]>("check_content_updates", { instanceId, kind }),
  updateContent: (instanceId: string, kind: ContentKind, fileNames: string[]) =>
    invoke<number>("update_content", { instanceId, kind, fileNames }),
  openContentFolder: (instanceId: string, kind: ContentKind) => invoke<void>("open_content_folder", { instanceId, kind }),

  // configs
  listConfigs: (instanceId: string) => invoke<ConfigFile[]>("list_configs", { instanceId }),
  readConfig: (instanceId: string, path: string) => invoke<string>("read_config", { instanceId, path }),
  writeConfig: (instanceId: string, path: string, content: string) =>
    invoke<void>("write_config", { instanceId, path, content }),

  // otimização
  curatedMods: (instanceId: string, pack: "performance" | "pvp") => invoke<CuratedStatus[]>("curated_mods", { instanceId, pack }),
  installCurated: (instanceId: string, keys: string[]) => invoke<InstallReport>("install_curated", { instanceId, keys }),
  applyGraphicsPreset: (instanceId: string, preset: GraphicsPreset) =>
    invoke<void>("apply_graphics_preset", { instanceId, preset }),

  // cosméticos
  getWardrobe: () => invoke<Wardrobe>("get_wardrobe"),
  importCosmetic: (path: string, kind: CosmeticKind, name: string, model: SkinModel | null) =>
    invoke<Wardrobe>("import_cosmetic", { path, kind, name, model }),
  removeCosmetic: (id: string) => invoke<Wardrobe>("remove_cosmetic", { id }),
  setCosmeticModel: (id: string, model: SkinModel) => invoke<Wardrobe>("set_cosmetic_model", { id, model }),
  equipCosmetic: (accountId: string, kind: CosmeticKind, id: string | null) =>
    invoke<Wardrobe>("equip_cosmetic", { accountId, kind, id }),
  cosmeticDataUrl: (id: string) => invoke<string>("cosmetic_data_url", { id }),
  uploadSkinToMojang: (accountId: string, cosmeticId: string) =>
    invoke<void>("upload_skin_to_mojang", { accountId, cosmeticId }),
  mojangCapes: (accountId: string) => invoke<MojangCape[]>("mojang_capes", { accountId }),
  setMojangCape: (accountId: string, capeId: string | null) => invoke<void>("set_mojang_cape", { accountId, capeId }),
  syncCosmetics: (accountId: string) => invoke<void>("sync_cosmetics", { accountId }),
  importCosmeticData: (pngBase64: string, kind: CosmeticKind, name: string, model: SkinModel | null) =>
    invoke<Cosmetic>("import_cosmetic_data", { pngBase64, kind, name, model }),
  setItems3d: (accountId: string, items: string[]) => invoke<Wardrobe>("set_items3d", { accountId, items }),
  cosmeticsModStatus: () => invoke<CosmeticsModStatus>("cosmetics_mod_status"),
  galleryList: (query: string, offset: number) => invoke<GalleryPage>("gallery_list", { query, offset }),
  galleryImport: (id: string) => invoke<Wardrobe>("gallery_import", { id }),
  galleryPublish: (accountId: string, cosmeticId: string, name: string) =>
    invoke<void>("gallery_publish", { accountId, cosmeticId, name }),
};

export const events = {
  onProgress: (cb: (e: ProgressEvent) => void): Promise<UnlistenFn> =>
    listen<ProgressEvent>("launch://progress", (e) => cb(e.payload)),
  onLogs: (cb: (lines: LogLine[]) => void): Promise<UnlistenFn> =>
    listen<LogLine[]>("launch://log", (e) => cb(e.payload)),
  onExit: (cb: (e: ExitInfo) => void): Promise<UnlistenFn> => listen<ExitInfo>("launch://exit", (e) => cb(e.payload)),
};

export function avatarUrl(account: Pick<Account, "id" | "username" | "kind">, size = 64): string {
  // Contas Microsoft usam o UUID (skin real). Offline usa o nick, que é o que o
  // CustomSkinLoader resolve como fallback via Mojang.
  const key = account.kind === "microsoft" ? account.id.replace(/-/g, "") : account.username;
  return `https://mc-heads.net/avatar/${encodeURIComponent(key)}/${size}`;
}

export function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)} mi`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(0)} mil`;
  return String(n);
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let v = bytes / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${units[i]}`;
}
