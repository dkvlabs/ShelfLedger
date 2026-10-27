import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  BookOpen,
  LayoutGrid,
  List,
  Plus,
  ScanBarcode,
  Search,
  Upload,
  ChevronRight,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { api } from "@/lib/api";
import { useT } from "@/lib/i18n";
import AddBookDialog from "./AddBookDialog";

export default function LibraryView({
  onOpenWork,
}: {
  onOpenWork: (id: string) => void;
}) {
  const t = useT();
  const [input, setInput] = useState("");
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<"grid" | "table">("grid");
  const [addOpen, setAddOpen] = useState(false);
  const [scan, setScan] = useState(false);
  const [sort, setSort] = useState("added");
  const [filter, setFilter] = useState("all");
  useEffect(() => {
    const id = setTimeout(() => setQuery(input.trim()), 250);
    return () => clearTimeout(id);
  }, [input]);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "n") {
        e.preventDefault();
        setScan(false);
        setAddOpen(true);
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, []);
  const {
    data: works,
    isPending,
    error,
  } = useQuery({
    queryKey: ["works", query],
    queryFn: () => api.searchWorks(query || null),
  });
  const { data: stats } = useQuery({
    queryKey: ["stats"],
    queryFn: api.getLibraryStats,
  });
  const sorted = useMemo(() => {
    const list = (works ?? []).filter(
      (w) =>
        filter === "all" ||
        (filter === "owned" ? w.copyCount > 0 : w.copyCount === 0),
    );
    if (sort !== "added")
      list.sort((a, b) =>
        (sort === "title" ? a.title : a.authorNames.join(", ")).localeCompare(
          sort === "title" ? b.title : b.authorNames.join(", "),
        ),
      );
    return list;
  }, [works, sort, filter]);
  const add = (scanning = false) => {
    setScan(scanning);
    setAddOpen(true);
  };
  const openImport = () =>
    window.dispatchEvent(new CustomEvent("shelfledger:open-import"));
  return (
    <div className="library-page">
      <header className="library-heading">
        <div>
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            {t("Your collection")}
            <ChevronRight size={14} />
          </div>
          <h1>{t("The library.")}</h1>
          <p className="font-display library-subtitle">
            {t("Every book, a story worth keeping.")}
          </p>
        </div>
        <div className="flex gap-3">
          <Button className="h-12" variant="outline" onClick={() => add(true)}>
            <ScanBarcode />
            {t("Scan ISBN")}
          </Button>
          <Button className="h-12" onClick={() => add()}>
            <Plus />
            {t("Add book")}
          </Button>
        </div>
      </header>
      <img
        className="library-atmosphere"
        src="/images/library-atmosphere.png"
        alt=""
        width="2172"
        height="724"
      />
      <div className="library-toolbar">
        <div className="mr-auto flex gap-4 whitespace-nowrap text-xs text-muted-foreground">
          <span>
            {stats?.workCount ?? 0}{" "}
            {t(stats?.workCount === 1 ? "work" : "works")}
          </span>
          <span className="border-l pl-4">
            {stats?.copyCount ?? 0}{" "}
            {t(stats?.copyCount === 1 ? "copy" : "copies")}
          </span>
        </div>
        <div className="search-field">
          <Search />
          <Input
            aria-label={t("Search your library")}
            placeholder={t("Search title, author, ISBN…")}
            value={input}
            onChange={(e) => setInput(e.target.value)}
          />
        </div>
        <Select value={filter} onValueChange={setFilter}>
          <SelectTrigger className="h-11 w-36" aria-label={t("Filter books")}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">{t("All books")}</SelectItem>
            <SelectItem value="owned">{t("With copies")}</SelectItem>
            <SelectItem value="unowned">{t("Without copies")}</SelectItem>
          </SelectContent>
        </Select>
        <Select value={sort} onValueChange={setSort}>
          <SelectTrigger className="h-11 w-36" aria-label={t("Sort order")}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="added">{t("Recently added")}</SelectItem>
            <SelectItem value="title">{t("Title")}</SelectItem>
            <SelectItem value="author">{t("Author")}</SelectItem>
          </SelectContent>
        </Select>
        <div className="flex rounded-md border">
          <Button
            variant="ghost"
            className={
              mode === "grid" ? "text-primary" : "text-muted-foreground"
            }
            aria-label={t("Grid view")}
            aria-pressed={mode === "grid"}
            onClick={() => setMode("grid")}
          >
            <LayoutGrid />
          </Button>
          <Button
            variant="ghost"
            className={
              mode === "table" ? "text-primary" : "text-muted-foreground"
            }
            aria-label={t("Table view")}
            aria-pressed={mode === "table"}
            onClick={() => setMode("table")}
          >
            <List />
          </Button>
        </div>
      </div>
      {error && (
        <p role="alert" className="text-destructive">
          {(error as Error).message}
        </p>
      )}
      {isPending ? (
        <p role="status">{t("Loading…")}</p>
      ) : sorted.length === 0 ? (
        <div className="library-empty">
          <BookOpen className="mb-1 h-16 w-16 text-primary" strokeWidth={0.8} />
          <h2 className="font-display">
            {t(
              query || filter !== "all"
                ? "No books match your search"
                : "Make room for your next chapter.",
            )}
          </h2>
          <p className="text-muted-foreground">
            {t(
              query || filter !== "all"
                ? "Try a different title, author, or ISBN."
                : "Add your first book, scan an ISBN, or bring your collection from Goodreads.",
            )}
          </p>
          {!query && filter === "all" && (
            <div className="mt-2 flex justify-center gap-3">
              <Button className="h-12 px-7" onClick={() => add()}>
                <Plus />
                {t("Add a book")}
              </Button>
              <Button
                className="h-12 px-6"
                variant="outline"
                onClick={openImport}
              >
                <Upload />
                {t("Import collection")}
              </Button>
            </div>
          )}
        </div>
      ) : mode === "grid" ? (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-6">
          {sorted.map((w) => (
            <button
              key={w.id}
              className="library-book flex flex-col justify-between p-7 text-left transition duration-300"
              onClick={() => onOpenWork(w.id)}
            >
              <span>
                <span className="mb-4 block text-xs text-primary">
                  {w.authorNames.join(", ") || t("Unknown author")}
                </span>
                <span className="book-title line-clamp-4 block">{w.title}</span>
                {w.subtitle && (
                  <span className="mt-3 line-clamp-2 block text-xs text-muted-foreground">
                    {w.subtitle}
                  </span>
                )}
              </span>
              <span className="mt-6 border-t border-primary/20 pt-3 text-xs text-muted-foreground">
                {w.copyCount} {t(w.copyCount === 1 ? "copy" : "copies")}
                <ChevronRight size={15} className="float-right text-primary" />
              </span>
            </button>
          ))}
        </div>
      ) : (
        <div className="responsive-table">
          <table className="w-full text-left text-sm">
            <thead>
              <tr className="border-b text-muted-foreground">
                <th>{t("Title")}</th>
                <th>{t("Authors")}</th>
                <th>{t("Editions")}</th>
                <th>{t("Copies")}</th>
              </tr>
            </thead>
            <tbody>
              {sorted.map((w) => (
                <tr key={w.id} className="border-b hover:bg-accent">
                  <td>
                    <button
                      className="text-left font-medium underline-offset-4 hover:underline"
                      onClick={() => onOpenWork(w.id)}
                    >
                      {w.title}
                    </button>
                  </td>
                  <td>{w.authorNames.join(", ")}</td>
                  <td>{w.editionCount}</td>
                  <td>{w.copyCount}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {addOpen && (
        <AddBookDialog
          key={scan ? "scan" : "manual"}
          initialScan={scan}
          open={addOpen}
          onOpenChange={setAddOpen}
          onCreated={onOpenWork}
        />
      )}
    </div>
  );
}
