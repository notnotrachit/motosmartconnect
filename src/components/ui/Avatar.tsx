import { User } from "lucide-react";
import { cn } from "@/lib/cn";
import { initial, nameHue } from "@/lib/format";

interface AvatarProps {
  name: string;
  /** Show a person glyph instead of the first letter (e.g. for bare phone numbers). */
  anonymous?: boolean;
  size?: number;
  className?: string;
}

/** Round contact avatar with a stable tint per name. */
export function Avatar({ name, anonymous = false, size = 40, className }: AvatarProps) {
  const h = nameHue(name || "?");
  return (
    <span
      aria-hidden="true"
      className={cn("grid shrink-0 place-items-center rounded-full font-semibold text-white", className)}
      style={{
        width: size,
        height: size,
        fontSize: size * 0.4,
        background: `linear-gradient(150deg, hsl(${h} 58% 60%), hsl(${(h + 32) % 360} 54% 46%))`,
        boxShadow: "inset 0 1px 0 rgb(255 255 255 / 0.22)",
      }}
    >
      {anonymous ? <User size={size * 0.5} strokeWidth={2.2} /> : initial(name)}
    </span>
  );
}
