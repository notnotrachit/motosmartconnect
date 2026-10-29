import { MirrorController } from "@/rtc/MirrorController";
import { toast } from "@/store/toast";

let instance: MirrorController | null = null;

/** The single phone-screen controller for the main window. */
export function mirror(): MirrorController {
  instance ??= new MirrorController((message) => toast.error(message));
  return instance;
}
