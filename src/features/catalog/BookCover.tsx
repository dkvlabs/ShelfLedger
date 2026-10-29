import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "@/lib/api";
import { useT } from "@/lib/i18n";

export default function BookCover({ path, title, className = "" }: {
  path: string | null; title: string; className?: string;
}) {
  const tx = useT();
  const ref = useRef<HTMLSpanElement>(null);
  const [visible, setVisible] = useState(typeof IntersectionObserver === "undefined");
  const [failedPath, setFailedPath] = useState<string | null>(null);
  useEffect(() => {
    if (visible || !ref.current || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(([entry]) => {
      if (entry.isIntersecting) { setVisible(true); observer.disconnect(); }
    }, { rootMargin: "200px" });
    observer.observe(ref.current);
    return () => observer.disconnect();
  }, [visible]);
  const { data } = useQuery({
    queryKey: ["cover", path], queryFn: () => api.getCover(path!),
    enabled: !!path && path.startsWith("covers/") && visible, staleTime: Infinity, retry: false,
  });
  const src = failedPath !== path && data?.dataUrl.startsWith("data:image/jpeg;base64,") ? data.dataUrl : null;
  return <span ref={ref} className={`relative flex aspect-[2/3] items-center justify-center overflow-hidden rounded-md bg-black/15 ${className}`}>
    {src ? <img src={src} alt={tx("Cover of {value}", { value: title })} decoding="async" loading="lazy"
      className="h-full w-full object-contain" onError={() => setFailedPath(path)} />
      : <span className="p-4 text-center font-display text-lg text-muted-foreground">{title}</span>}
  </span>;
}
