import { cn } from "@/lib/cn";
import { initial, nameHue as hue } from "@/lib/format";

interface AppIconProps {
  src?: string;
  name: string;
  size?: number;
  className?: string;
}

/** App icon from the phone, or a tinted letter tile while icons load. */
export function AppIcon({ src, name, size = 40, className }: AppIconProps) {
  const radius = Math.round(size * 0.26);
  if (src) {
    return (
      <img
        src={src}
        alt=""
        draggable={false}
        className={cn("shrink-0 object-contain", className)}
        style={{ width: size, height: size, borderRadius: radius }}
      />
    );
  }
  const h = hue(name || "?");
  return (
    <span
      aria-hidden="true"
      className={cn("grid shrink-0 place-items-center font-semibold text-white", className)}
      style={{
        width: size,
        height: size,
        borderRadius: radius,
        fontSize: size * 0.42,
        background: `linear-gradient(145deg, hsl(${h} 62% 58%), hsl(${(h + 28) % 360} 58% 44%))`,
        boxShadow: "inset 0 1px 0 rgb(255 255 255 / 0.2)",
      }}
    >
      {initial(name)}
    </span>
  );
}
