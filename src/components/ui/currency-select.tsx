import { currencies, currencyName } from "@/lib/format";
import { useLanguage } from "@/lib/i18n";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./select";

/** Type a code while open to jump to a currency. Existing legacy codes stay editable. */
export function CurrencySelect({ value, onValueChange, id, disabled }: {
  value: string; onValueChange: (value: string) => void; id: string; disabled?: boolean;
}) {
  const language = useLanguage();
  return <Select value={value} onValueChange={onValueChange} disabled={disabled}>
    <SelectTrigger id={id}><SelectValue /></SelectTrigger>
    <SelectContent>
      {Array.from(new Set([...currencies, value])).filter(Boolean).sort().map((code) =>
        <SelectItem key={code} value={code} textValue={`${code} ${currencyName(code, language)}`}>
          <bdi>{code}</bdi> — {currencyName(code, language)}
        </SelectItem>)}
    </SelectContent>
  </Select>;
}
