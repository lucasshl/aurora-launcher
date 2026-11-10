// Espelho dos tipos serializados pelo launcher-core (serde camelCase).

export type AccountKind = "microsoft" | "offline";

export interface Account {
  id: string;
  username: string;
  kind: AccountKind;
  skinUrl: string | null;
  addedAt: number;
}

export interface AccountsView {
  active: string | null;
  accounts: Account[];
}

export interface DeviceCode {
  userCode: string;
  deviceCode: string;
  verificationUri: string;
  expiresIn: number;
  interval: number;
  message?: string | null;
}

export type ModLoader = "vanilla" | "fabric" | "forge" | "neoforge";

export interface Resolution {
  width: number;
  height: number;
}

export interface InstanceConfig {
  id: string;
  name: string;
  minecraftVersion: string;
  loader: ModLoader;
  loaderVersion: string | null;
  versionId: string | null;
  javaPath: string | null;
  memoryMinMb: number;
  memoryMaxMb: number;
  jvmArgs: string[];
  optimizedGc: boolean;
  skinLoader: boolean;
  experimentalCds: boolean;
  resolution: Resolution | null;
  createdAt: number;
  lastPlayed: number | null;
}

export interface NewInstance {
  name: string;
  minecraftVersion: string;
  loader: ModLoader;
  loaderVersion?: string | null;
  memoryMaxMb?: number | null;
}

export type Stage =
  | "manifest"
  | "loader"
  | "java"
  | "client"
  | "libraries"
  | "asset_index"
  | "assets"
  | "mods"
  | "natives"
  | "launching";

export interface ProgressEvent {
  instanceId: string;
  stage: Stage;
  filesDone: number;
  filesTotal: number;
  bytesDone: number;
  bytesTotal: number;
  current: string | null;
}

export type LogLevel = "info" | "warn" | "error" | "debug";

export interface LogLine {
  instanceId: string;
  stream: "stdout" | "stderr" | "launcher";
  level: LogLevel;
  line: string;
}

export interface ExitInfo {
  instanceId: string;
  code: number | null;
  crashed: boolean;
  crashReport: string | null;
}

export interface LaunchStarted {
  instanceId: string;
  pid: number | null;
  java: string;
  javaMajor: number;
  versionId: string;
}

export interface Settings {
  msClientId: string | null;
  customSkinApi: string | null;
  cosmeticsServer: string | null;
  downloadConcurrency: number;
  defaultMemoryMaxMb: number;
  defaultMemoryMinMb: number;
  keepConsoleAttached: boolean;
}

export interface SystemInfo {
  totalMemoryMb: number;
  dataDir: string;
  hasMsClientId: boolean;
  builtInMsClientId: boolean;
}

export interface GameVersion {
  id: string;
  kind: "release" | "snapshot" | "old_beta" | "old_alpha" | string;
  releaseTime: string;
}

// ---------------------------------------------------------------- conteúdo / Modrinth

export type ContentKind = "mod" | "resourcepack" | "shader";
export type SortIndex = "relevance" | "downloads" | "follows" | "newest" | "updated";

export interface ModMeta {
  id: string | null;
  name: string | null;
  version: string | null;
  description: string | null;
  authors: string[];
  icon: string | null;
}

export interface InstalledContent {
  fileName: string;
  enabled: boolean;
  kind: ContentKind;
  size: number;
  meta: ModMeta;
  projectId: string | null;
  versionId: string | null;
  managed: boolean;
}

export interface SearchHit {
  projectId: string;
  slug: string;
  title: string;
  description: string;
  author: string;
  downloads: number;
  iconUrl: string | null;
  categories: string[];
  dateModified: string;
}

export interface SearchResult {
  hits: SearchHit[];
  totalHits: number;
  offset: number;
  limit: number;
}

export interface ModrinthVersion {
  id: string;
  projectId: string;
  name: string;
  versionNumber: string;
  gameVersions: string[];
  loaders: string[];
  versionType: string;
  datePublished: string;
  files: { url: string; filename: string; primary: boolean; size: number }[];
}

export interface UpdateInfo {
  fileName: string;
  projectId: string;
  currentVersion: string;
  latest: ModrinthVersion;
}

export interface InstallReport {
  installed: string[];
  skipped: string[];
}

export interface ConfigFile {
  path: string;
  size: number;
  modified: number;
}

// ---------------------------------------------------------------- otimização

export type CuratedCategory =
  | "render" | "logic" | "memory" | "worldgen" | "startup" | "network" | "pvp" | "hud" | "shaders" | "utility";

export interface CuratedStatus {
  key: string;
  candidates: string[];
  name: string;
  description: string;
  category: CuratedCategory;
  recommended: boolean;
  availableSlug: string | null;
  version: string | null;
  installed: boolean;
}

export type GraphicsPreset = "potato" | "balanced" | "quality";

// ---------------------------------------------------------------- cosméticos

export type CosmeticKind = "skin" | "cape";
export type SkinModel = "classic" | "slim";

export interface Cosmetic {
  id: string;
  kind: CosmeticKind;
  name: string;
  model: SkinModel;
  sha256: string;
  addedAt: number;
}

export interface Wardrobe {
  items: Cosmetic[];
  equipped: Record<string, { skin: string | null; cape: string | null; items3d?: string[] } | undefined>;
}

export interface MojangCape {
  id: string;
  state: "ACTIVE" | "INACTIVE" | string;
  url: string;
  alias: string | null;
}

export interface GalleryItem {
  id: string;
  name: string;
  author: string;
  model: SkinModel;
  downloads: number;
  texture: string;
}

export interface GalleryPage {
  items: GalleryItem[];
  total: number;
}

export interface CosmeticsModStatus {
  built: boolean;
  path: string;
  minecraftVersion: string;
}
