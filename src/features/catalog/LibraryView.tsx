import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { BookPlus, LayoutGrid, Library, List } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { api } from "@/lib/api";
import { cn } from "@/lib/utils";
import AddBookDialog from "./AddBookDialog";

function WorkCard({
  title,
  subtitle,
  authorNames,
  copyCount,
  onClick,
}: {
  title: string;
  subtitle: string | null;
  authorNames: string[];
  copyCount: number;
  onClick: () => void;
}) {
  return (
    <Card
      className="cursor-pointer transition-colors hover:bg-accent"
      role="button"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        }
      }}
    >
      <CardContent className="flex h-44 flex-col justify-between p-4">
        <div>
          <div className="line-clamp-2 font-medium leading-snug">{title}</div>
          {subtitle && <div className="line-clamp-1 text-xs text-muted-foreground">{subtitle}</div>}
          <div className="mt-1 line-clamp-1 text-sm text-muted-foreground">
            {authorNames.join(", ") || "Unknown author"}
          </div>
        </div>
        <div>
          <Badge variant={copyCount > 0 ? "default" : "secondary"}>
            {copyCount === 0
              ? "No copies"
              : copyCount === 1
                ? "1 copy"
                : `${copyCount} copies`}
          </Badge>
        </div>
      </CardContent>
    </Card>
  );
}

export default function LibraryView({
  onOpenWork,
}: {
  onOpenWork: (workId: string) => void;
}) {
  const [searchInput, setSearchInput] = useState("");
  const [query, setQuery] = useState("");
  const [viewMode, setViewMode] = useState<"grid" | "table">("grid");
  const [addOpen, setAddOpen] = useState(false);
  const [sortBy, setSortBy] = useState<"added" | "title" | "author">("added");

  // Debounce search input.
  useEffect(() => {
    const t = setTimeout(() => setQuery(searchInput.trim()), 250);
    return () => clearTimeout(t);
  }, [searchInput]);

  // Keyboard shortcut: Ctrl/Cmd+N opens Add Book (docs/design.md).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "n") {
        e.preventDefault();
        setAddOpen(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const { data: works, isPending, isError, error } = useQuery({
    queryKey: ["works", query],
    queryFn: () => api.searchWorks(query || null),
  });

  // Client-side sort of the loaded list. Server order (relevance/recent)
  // is preserved for "added"; title/author sort is locale-aware.
  const sorted = useMemo(() => {
    const list = [...(works ?? [])];
    if (sortBy === "title") {
      list.sort((a, b) => a.title.localeCompare(b.title));
    } else if (sortBy === "author") {
      list.sort((a, b) => {
        const aa = a.authorNames.join(", ");
        const bb = b.authorNames.join(", ");
        return aa.localeCompare(bb) || a.title.localeCompare(b.title);
      });
    }
    return list;
  }, [works, sortBy]);

  return (
    <div className="p-6">
      <div className="mb-4 flex items-center justify-between gap-3">
        <h1 className="text-xl font-semibold">My Library</h1>
        <div className="flex items-center gap-2">
          <Input
            aria-label="Search your library"
            placeholder="Search title, author, ISBN…"
            value={searchInput}
            onChange={(e) => setSearchInput(e.target.value)}
            className="w-72"
          />
          <Select value={sortBy} onValueChange={(v) => setSortBy(v as typeof sortBy)}>
            <SelectTrigger className="w-36" aria-label="Sort order">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="added">Recently added</SelectItem>
              <SelectItem value="title">Title</SelectItem>
              <SelectItem value="author">Author</SelectItem>
            </SelectContent>
          </Select>
          <Button
            variant="outline"
            size="icon"
            aria-label={viewMode === "grid" ? "Switch to table view" : "Switch to grid view"}
            onClick={() => setViewMode(viewMode === "grid" ? "table" : "grid")}
          >
            {viewMode === "grid" ? <List /> : <LayoutGrid />}
          </Button>
          <Button onClick={() => setAddOpen(true)}>
            <BookPlus /> Add Book
          </Button>
        </div>
      </div>

      {isError && (
        <p role="alert" className="rounded-md border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
          Could not load your library: {(error as Error).message}
        </p>
      )}

      {isPending ? (
        <p className="text-sm text-muted-foreground">Loading…</p>
      ) : sorted.length === 0 ? (
        <div className="mt-16 flex flex-col items-center gap-3 text-center">
          <Library className="h-10 w-10 text-muted-foreground" />
          <h2 className="text-lg font-semibold">
            {query ? "No books match your search" : "Welcome to ShelfLedger"}
          </h2>
          <p className="max-w-sm text-sm text-muted-foreground">
            {query
              ? "Try a different title, author, or ISBN."
              : "Add your first book by ISBN or manually — everything stays on this computer."}
          </p>
          {!query && (
            <Button onClick={() => setAddOpen(true)}>
              <BookPlus /> Add your first book
            </Button>
          )}
        </div>
      ) : viewMode === "grid" ? (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-4">
          {sorted.map((w) => (
            <WorkCard
              key={w.id}
              title={w.title}
              subtitle={w.subtitle}
              authorNames={w.authorNames}
              copyCount={w.copyCount}
              onClick={() => onOpenWork(w.id)}
            />
          ))}
        </div>
      ) : (
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b text-left text-muted-foreground">
              <th className="py-2 pr-3 font-medium">Title</th>
              <th className="py-2 pr-3 font-medium">Authors</th>
              <th className="py-2 pr-3 font-medium">Editions</th>
              <th className="py-2 font-medium">Copies</th>
            </tr>
          </thead>
          <tbody>
            {sorted.map((w) => (
              <tr
                key={w.id}
                className="cursor-pointer border-b transition-colors hover:bg-accent"
                tabIndex={0}
                onClick={() => onOpenWork(w.id)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") onOpenWork(w.id);
                }}
              >
                <td className="py-2 pr-3 font-medium">{w.title}</td>
                <td className="py-2 pr-3 text-muted-foreground">{w.authorNames.join(", ")}</td>
                <td className="py-2 pr-3">{w.editionCount}</td>
                <td className={cn("py-2")}>{w.copyCount}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <AddBookDialog open={addOpen} onOpenChange={setAddOpen} onCreated={(id) => onOpenWork(id)} />
    </div>
  );
}
