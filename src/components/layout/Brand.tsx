import { cn } from "@/lib/cn";

export function BrandMark({ size = 22, className }: { size?: number; className?: string }) {
  return (
    <img
      data-tauri-drag-region
      src="/moto-smart-connect.svg"
      alt=""
      draggable={false}
      className={cn("shrink-0", className)}
      width={size}
      height={size}
    />
  );
}

export function Brand() {
  return (
    <div data-tauri-drag-region className="flex items-center gap-2">
      <BrandMark />
      <span data-tauri-drag-region className="font-display text-[0.8125rem] font-semibold tracking-tight text-fg">
        Moto Smart Connect
      </span>
    </div>
  );
}
