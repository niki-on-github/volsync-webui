import type { App } from "@/types";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface Props {
  selected: App | null;
  apps: App[];
  onSelect: (app: App | null) => void;
}

export function AppSelector({ selected, apps, onSelect }: Props) {
  return (
    <div className="flex items-center gap-2">
      <label className="text-sm font-medium text-muted-foreground">
        Application:
      </label>
      <Select
        value={
          selected ? `${selected.name}/${selected.namespace}` : ""
        }
        onValueChange={(val) => {
          const app =
            apps.find((a) => `${a.name}/${a.namespace}` === val) ?? null;
          onSelect(app);
        }}
      >
        <SelectTrigger className="w-[250px]">
          <SelectValue placeholder="Select an app..." />
        </SelectTrigger>
        <SelectContent>
          {apps.length === 0 && (
            <div className="py-2 px-3 text-sm text-muted-foreground">
              No apps found
            </div>
          )}
          {apps.map((app) => {
            const val = `${app.name}/${app.namespace}`;
            return (
              <SelectItem key={val} value={val}>
                {app.name}
                <span className="ml-2 text-muted-foreground">
                  ({app.namespace})
                </span>
              </SelectItem>
            );
          })}
        </SelectContent>
      </Select>
    </div>
  );
}
