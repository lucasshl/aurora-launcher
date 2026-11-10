import { useEffect, useRef, useState } from "react";
import { Check, Copy, ExternalLink, ShieldCheck, WifiOff } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errorMessage } from "../lib/api";
import type { Account, DeviceCode } from "../lib/types";
import { Button, Field, Modal, Spinner, cx, inputClass } from "./ui";

interface Props {
  open: boolean;
  onClose: () => void;
  onAdded: (account: Account) => void;
  hasClientId: boolean;
  onOpenSettings: () => void;
  initialTab?: "microsoft" | "offline";
}

type Tab = "microsoft" | "offline";
const NICK_RE = /^[A-Za-z0-9_]{3,16}$/;
const AZURE_URL = "https://portal.azure.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade";
const MOJANG_FORM_URL = "https://aka.ms/mce-reviewappid";

export function AddAccountModal({ open, onClose, onAdded, hasClientId, onOpenSettings, initialTab }: Props) {
  const [tab, setTab] = useState<Tab>(initialTab ?? "microsoft");
  useEffect(() => {
    if (open && initialTab) setTab(initialTab);
  }, [open, initialTab]);

  return (
    <Modal open={open} onClose={onClose} title="Adicionar conta">
      <div className="mb-5 grid grid-cols-2 gap-1 rounded-lg bg-bg p-1">
        {(
          [
            ["microsoft", "Original (Microsoft)", ShieldCheck],
            ["offline", "Pirata (offline)", WifiOff],
          ] as const
        ).map(([id, label, Icon]) => (
          <button
            key={id}
            onClick={() => setTab(id)}
            className={cx(
              "flex items-center justify-center gap-2 rounded-md py-2 text-sm font-medium transition",
              tab === id ? "bg-panel-3 text-white shadow" : "text-muted hover:text-zinc-200",
            )}
          >
            <Icon size={15} /> {label}
          </button>
        ))}
      </div>
      {tab === "microsoft" ? (
        <MicrosoftLogin
          active={open}
          hasClientId={hasClientId}
          onOpenSettings={onOpenSettings}
          onDone={(a) => {
            onAdded(a);
            onClose();
          }}
        />
      ) : (
        <OfflineLogin
          onDone={(a) => {
            onAdded(a);
            onClose();
          }}
        />
      )}
    </Modal>
  );
}

function OfflineLogin({ onDone }: { onDone: (a: Account) => void }) {
  const [nick, setNick] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const valid = NICK_RE.test(nick);

  const submit = async () => {
    if (!valid) return;
    setBusy(true);
    setError(null);
    try {
      onDone(await api.addOfflineAccount(nick));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
      className="space-y-4"
    >
      <Field
        label="Nick"
        hint={
          nick && !valid
            ? "Use 3–16 caracteres: letras, números ou _"
            : "Funciona em singleplayer e em servidores com online-mode=false."
        }
      >
        <input
          autoFocus
          value={nick}
          maxLength={16}
          onChange={(e) => setNick(e.target.value.trim())}
          placeholder="Steve"
          className={cx(inputClass, nick && !valid && "border-err focus:border-err")}
        />
      </Field>
      <p className="rounded-md bg-bg px-3 py-2 text-xs text-muted">
        O UUID é gerado igual ao do servidor (<code>OfflinePlayer:{nick || "nick"}</code>), então seus
        inventários e permissões se mantêm entre sessões. Skins são carregadas pelo CustomSkinLoader em instâncias com
        Fabric/Forge.
      </p>
      {error && <p className="text-sm text-err">{error}</p>}
      <Button type="submit" variant="primary" disabled={!valid || busy} className="w-full">
        {busy ? <Spinner /> : "Criar conta offline"}
      </Button>
    </form>
  );
}

function MicrosoftLogin({
  active,
  hasClientId,
  onOpenSettings,
  onDone,
}: {
  active: boolean;
  hasClientId: boolean;
  onOpenSettings: () => void;
  onDone: (a: Account) => void;
}) {
  const [code, setCode] = useState<DeviceCode | null>(null);
  const [status, setStatus] = useState<"idle" | "requesting" | "waiting" | "error">("idle");
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [remaining, setRemaining] = useState(0);
  const codeRef = useRef<DeviceCode | null>(null);

  // Cancela o polling no backend se o modal fechar no meio do login
  useEffect(() => {
    if (active) return;
    if (codeRef.current) void api.cancelMicrosoftLogin(codeRef.current.deviceCode);
    codeRef.current = null;
    setCode(null);
    setStatus("idle");
  }, [active]);

  useEffect(() => {
    if (!code) return;
    const end = Date.now() + code.expiresIn * 1000;
    const t = setInterval(() => setRemaining(Math.max(0, Math.round((end - Date.now()) / 1000))), 1000);
    return () => clearInterval(t);
  }, [code]);

  const start = async () => {
    setStatus("requesting");
    setError(null);
    try {
      const dc = await api.beginMicrosoftLogin();
      codeRef.current = dc;
      setCode(dc);
      setStatus("waiting");
      await navigator.clipboard.writeText(dc.userCode).catch(() => undefined);
      await openUrl(dc.verificationUri).catch(() => undefined);
      const account = await api.completeMicrosoftLogin(dc);
      codeRef.current = null;
      onDone(account);
    } catch (e) {
      const msg = errorMessage(e);
      if (!msg.includes("cancelado")) {
        setError(msg);
        setStatus("error");
      }
    }
  };

  if (!hasClientId) {
    return (
      <div className="space-y-4 text-sm">
        <p className="text-muted">
          O login original usa o Device Code Flow da Microsoft. É preciso um app no Azure (grátis) e a liberação da Mojang para
          a API do Minecraft:
        </p>
        <ol className="list-decimal space-y-2 pl-5 text-zinc-300">
          <li>
            Crie o app em <b>App registrations → New registration</b>, escolha "Personal Microsoft accounts only" e, em
            Authentication, ative <b>Allow public client flows</b>.
            <Button variant="subtle" className="mt-1.5 w-full" onClick={() => void openUrl(AZURE_URL)}>
              <ExternalLink size={14} /> Abrir portal do Azure
            </Button>
          </li>
          <li>
            Peça a liberação do Client ID para a Mojang (sem isso o login para com erro 403).
            <Button variant="subtle" className="mt-1.5 w-full" onClick={() => void openUrl(MOJANG_FORM_URL)}>
              <ExternalLink size={14} /> Abrir formulário da Mojang
            </Button>
          </li>
          <li>Cole o Application (client) ID em Configurações.</li>
        </ol>
        <Button variant="primary" className="w-full" onClick={onOpenSettings}>
          Abrir configurações
        </Button>
        <p className="text-xs text-muted">Enquanto isso, você pode jogar com uma conta pirata (aba ao lado).</p>
      </div>
    );
  }

  if (status === "waiting" && code) {
    return (
      <div className="space-y-5 text-center">
        <p className="text-sm text-muted">Acesse a página da Microsoft e digite o código abaixo:</p>
        <button
          onClick={() => {
            void navigator.clipboard.writeText(code.userCode);
            setCopied(true);
            setTimeout(() => setCopied(false), 1500);
          }}
          className="group mx-auto flex items-center gap-3 rounded-xl border border-line bg-bg px-6 py-4 font-mono text-3xl font-bold tracking-[0.25em] transition hover:border-accent"
        >
          {code.userCode}
          {copied ? <Check size={20} className="text-ok" /> : <Copy size={18} className="text-muted group-hover:text-white" />}
        </button>
        <Button variant="subtle" onClick={() => void openUrl(code.verificationUri)}>
          <ExternalLink size={15} /> {code.verificationUri.replace(/^https?:\/\//, "")}
        </Button>
        <div className="flex items-center justify-center gap-2 text-sm text-muted">
          <Spinner size={14} /> Aguardando confirmação no navegador… {Math.floor(remaining / 60)}:
          {String(remaining % 60).padStart(2, "0")}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted">
        Você entra pelo site oficial da Microsoft; o launcher nunca vê sua senha. O token de renovação fica no cofre
        seguro do sistema (Gerenciador de Credenciais do Windows / Keychain).
      </p>
      {error && <p className="selectable rounded-md bg-err/10 px-3 py-2 text-sm text-err">{error}</p>}
      <Button
        variant="primary"
        className="w-full bg-ms hover:bg-ms/85"
        disabled={status === "requesting"}
        onClick={() => void start()}
      >
        {status === "requesting" ? <Spinner /> : <ShieldCheck size={16} />} Entrar com Microsoft
      </Button>
    </div>
  );
}
