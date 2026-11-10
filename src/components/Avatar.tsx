import { useState } from "react";
import { avatarUrl } from "../lib/api";
import type { Account } from "../lib/types";
import { cx } from "./ui";

/** Cabeça do jogador renderizada (mc-heads.net) com fallback para a inicial do nick. */
export function Avatar({ account, size = 32, className }: { account: Account; size?: number; className?: string }) {
  const [failed, setFailed] = useState(false);
  return (
    <span
      style={{ width: size, height: size }}
      className={cx("relative inline-flex shrink-0 overflow-hidden rounded-md bg-panel-3", className)}
    >
      {failed ? (
        <span className="m-auto text-xs font-bold text-muted">{account.username.slice(0, 1).toUpperCase()}</span>
      ) : (
        <img
          src={avatarUrl(account, Math.max(32, size * 2))}
          alt={account.username}
          width={size}
          height={size}
          draggable={false}
          onError={() => setFailed(true)}
          style={{ imageRendering: "pixelated" }}
        />
      )}
    </span>
  );
}
