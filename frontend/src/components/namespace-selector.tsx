import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

interface Props {
  selected: string;
  namespaces: string[];
  onSelect: (ns: string) => void;
}

export function NamespaceSelector({ selected, namespaces, onSelect }: Props) {
  return (
    <div className="flex items-center gap-2">
      <label className="text-sm font-medium text-muted-foreground">
        Namespace:
      </label>
      <Select value={selected} onValueChange={onSelect}>
        <SelectTrigger className="w-[200px]">
          <SelectValue placeholder="All Namespaces" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="">All Namespaces</SelectItem>
          {namespaces.map((ns) => (
            <SelectItem key={ns} value={ns}>
              {ns}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}
