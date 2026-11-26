import {
  Battery,
  BatteryCharging,
  BatteryFull,
  BatteryLow,
  BatteryMedium,
  Bluetooth,
  BluetoothOff,
  Wifi,
  WifiOff,
  type LucideIcon,
} from "lucide-react";
import { Tooltip } from "@/components/ui/Tooltip";
import type { PhoneStatus } from "@/lib/types";

export interface StatusItem {
  key: string;
  label: string;
  value: string;
  icon: LucideIcon;
  tone: string;
}

function batteryIcon(status: PhoneStatus): LucideIcon {
  if (status.charging) return BatteryCharging;
  if (status.battery === null) return Battery;
  if (status.battery <= 20) return BatteryLow;
  if (status.battery <= 60) return BatteryMedium;
  return BatteryFull;
}

function batteryTone(status: PhoneStatus): string {
  if (status.charging) return "text-success";
  if (status.battery === null) return "text-fg-subtle";
  if (status.battery <= 10) return "text-danger";
  if (status.battery <= 20) return "text-warning";
  return "text-fg-muted";
}

function onOff(value: boolean | null): string {
  if (value === null) return "Unknown";
  return value ? "On" : "Off";
}

/** Rows for the phone's status. A value the phone has not sent yet reads "Unknown". */
export function phoneStatusItems(status: PhoneStatus): StatusItem[] {
  const battery =
    status.battery === null ? "Unknown" : status.charging ? `${status.battery}%, charging` : `${status.battery}%`;
  return [
    { key: "battery", label: "Battery", value: battery, icon: batteryIcon(status), tone: batteryTone(status) },
    {
      key: "wifi",
      label: "Wi-Fi",
      value: onOff(status.wifi),
      icon: status.wifi === false ? WifiOff : Wifi,
      tone: status.wifi === null ? "text-fg-subtle" : "text-fg-muted",
    },
    {
      key: "bluetooth",
      label: "Bluetooth",
      value: onOff(status.bluetooth),
      icon: status.bluetooth ? Bluetooth : BluetoothOff,
      tone: status.bluetooth === null ? "text-fg-subtle" : "text-fg-muted",
    },
  ];
}

/** Compact battery readout for the device pill. Renders nothing until the phone reports a level. */
export function PhoneBattery({ status }: { status: PhoneStatus }) {
  if (status.battery === null) return null;
  const Icon = batteryIcon(status);
  const label = `Phone battery ${status.battery}%${status.charging ? ", charging" : ""}`;
  return (
    <Tooltip label={label} side="bottom">
      <span className={`flex items-center gap-1 text-xs tabular-nums ${batteryTone(status)}`} aria-label={label}>
        <Icon size={13} aria-hidden="true" />
        {status.battery}%
      </span>
    </Tooltip>
  );
}
