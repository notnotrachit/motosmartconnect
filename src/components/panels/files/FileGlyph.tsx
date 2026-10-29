import { Archive, File, FileText, Folder, Image, Music, Video } from "lucide-react";
import { cn } from "@/lib/cn";
import { fileKind, type FileKind } from "@/lib/format";

const KINDS: Record<FileKind, { icon: typeof File; tint: string }> = {
  folder: { icon: Folder, tint: "bg-accent/15 text-accent" },
  image: { icon: Image, tint: "bg-sky-500/15 text-sky-500" },
  video: { icon: Video, tint: "bg-violet-500/15 text-violet-500" },
  audio: { icon: Music, tint: "bg-pink-500/15 text-pink-500" },
  archive: { icon: Archive, tint: "bg-amber-500/15 text-amber-500" },
  doc: { icon: FileText, tint: "bg-blue-500/15 text-blue-500" },
  file: { icon: File, tint: "bg-surface-3 text-fg-muted" },
};

export function FileGlyph({ name, directory = false, className }: { name: string; directory?: boolean; className?: string }) {
  const kind = KINDS[fileKind(name, directory)];
  const Icon = kind.icon;
  return (
    <span className={cn("grid size-8 shrink-0 place-items-center rounded-lg", kind.tint, className)}>
      <Icon size={15} />
    </span>
  );
}
