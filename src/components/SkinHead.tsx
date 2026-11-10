import { useEffect, useRef } from "react";
import { drawFace } from "../lib/skin";
import { cx } from "./ui";

/** Rosto 2D (frente da cabeça + camada externa) a partir de uma textura de skin. */
export function SkinHead({ src, size = 32, className }: { src: string; size?: number; className?: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const img = new Image();
    img.onload = () => {
      const ctx = ref.current?.getContext("2d");
      if (ctx) drawFace(ctx, img, size);
    };
    img.src = src;
  }, [src, size]);
  return <canvas ref={ref} width={size} height={size} className={cx("shrink-0 rounded-md bg-panel-3", className)} />;
}
