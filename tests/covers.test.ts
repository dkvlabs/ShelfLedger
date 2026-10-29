import { createElement } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import CoverEditor from "@/features/catalog/CoverEditor";
import BookCover from "@/features/catalog/BookCover";
import CoverCameraDialog from "@/features/catalog/CoverCameraDialog";
import { api, CommandError } from "@/lib/api";
import { readCoverPhoto, MAX_COVER_PHOTO_BYTES } from "@/lib/cover-photo";
import type { EditionDto } from "@/types/domain";

vi.mock("@/lib/api", async (original) => ({
  ...await original<typeof import("@/lib/api")>(),
  api: { getSettings: vi.fn(async () => ({ preferences: { language: "en" } })), getCover: vi.fn(), saveCoverPhoto: vi.fn(), fetchCover: vi.fn(), removeCover: vi.fn() },
}));
const path = "covers/photo.jpg";
const data = "data:image/jpeg;base64,/9j/2Q==";
const edition: EditionDto = {
  id: "edition-1", workId: "work-1", isbn13: "9786067882629", isbn10: null,
  publisher: null, publishedAt: null, language: "ro", pageCount: null,
  binding: null, coverPath: path, providerIdsJson: null, createdAt: "2026-10-29", copies: [],
};
beforeEach(() => {
  vi.mocked(api.getCover).mockResolvedValue({ dataUrl: data, source: "photo", createdAt: "2026-10-29" });
  vi.mocked(api.saveCoverPhoto).mockResolvedValue("covers/new-photo.jpg");
  vi.mocked(api.fetchCover).mockResolvedValue(null);
  vi.mocked(api.removeCover).mockResolvedValue();
});
afterEach(() => { cleanup(); vi.clearAllMocks(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });
function editor(input = edition) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  const invalidate = vi.spyOn(client, "invalidateQueries");
  render(createElement(QueryClientProvider, { client }, createElement(CoverEditor, { edition: input, title: "Mitologie nordica" })));
  return invalidate;
}
it("displays the saved local cover and its source without contacting a provider", async () => {
  editor();
  expect(await screen.findByRole("img", { name: "Cover of Mitologie nordica" })).toHaveAttribute("src", data);
  expect(screen.getByText(/Cover source: your photo/)).toBeInTheDocument();
  expect(api.fetchCover).not.toHaveBeenCalled();
});
it("saves selected photo bytes, refreshes details and the library, and allows selecting the same file again", async () => {
  const invalidate = editor();
  const file = new File([new Uint8Array([137, 80, 78, 71])], "cover.png", { type: "image/png" });
  fireEvent.change(screen.getByLabelText("Choose cover photo"), { target: { files: [file] } });
  await screen.findByText("Cover saved on this device.");
  expect(api.saveCoverPhoto).toHaveBeenCalledWith("edition-1", "data:image/png;base64,iVBORw==");
  expect(invalidate).toHaveBeenCalledWith({ queryKey: ["work"] });
  expect(invalidate).toHaveBeenCalledWith({ queryKey: ["works"] });
  fireEvent.change(screen.getByLabelText("Choose cover photo"), { target: { files: [file] } });
  await waitFor(() => expect(api.saveCoverPhoto).toHaveBeenCalledTimes(2));
});
it("only fetches on request and keeps the current cover when the ISBN has no image", async () => {
  editor(); await screen.findByRole("img");
  fireEvent.click(screen.getByRole("button", { name: "Fetch cover" }));
  await screen.findByText("No cover found for this ISBN. Choose or take a photo instead.");
  expect(api.fetchCover).toHaveBeenCalledExactlyOnceWith(edition.id);
  expect(screen.getByRole("img")).toHaveAttribute("src", data);
  expect(api.removeCover).not.toHaveBeenCalled();
});
it.each([
  ["metadata_disabled", "Online cover fetch is disabled on this device. Choose or take a photo instead."],
  ["network", "The cover provider is unavailable. Your saved cover is unchanged. Choose or take a photo instead."],
])("explains %s and keeps local photo actions available", async (kind, message) => {
  vi.mocked(api.fetchCover).mockRejectedValue(new CommandError(kind, "diagnostic"));
  editor(); fireEvent.click(screen.getByRole("button", { name: "Fetch cover" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(message);
  expect(screen.getByRole("button", { name: "Choose cover photo" })).toBeEnabled();
  expect(screen.getByRole("button", { name: "Photograph cover" })).toBeEnabled();
});
it("disables ISBN fetch without an ISBN and removes an existing cover explicitly", async () => {
  editor({ ...edition, isbn13: null });
  expect(screen.getByRole("button", { name: "Fetch cover" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "Remove cover" }));
  await screen.findByText("Cover removed.");
  expect(api.removeCover).toHaveBeenCalledWith(edition.id);
});
it("never uses remote cover paths as image sources", () => {
  const client = new QueryClient();
  render(createElement(QueryClientProvider, { client }, createElement(BookCover, { path: "https://evil.test/cover.jpg", title: "Local book" })));
  expect(screen.queryByRole("img")).not.toBeInTheDocument();
  expect(api.getCover).not.toHaveBeenCalled();
});
it("bounds photo input before IPC and handles document providers with no MIME type", async () => {
  await expect(readCoverPhoto(new File([], "empty.png"))).rejects.toThrow("10 MiB");
  const large = new File(["photo"], "large.jpg");
  Object.defineProperty(large, "size", { value: MAX_COVER_PHOTO_BYTES + 1 });
  await expect(readCoverPhoto(large)).rejects.toThrow("10 MiB");
  expect(await readCoverPhoto(new File(["jpeg"], "photo.jpg"))).toBe("data:image/jpeg;base64,anBlZw==");
});

function cameraSetup(getUserMedia = vi.fn()) {
  vi.stubGlobal("navigator", { mediaDevices: { getUserMedia } });
  vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue();
  const stop = vi.fn();
  const stream = { getTracks: () => [{ stop }] } as unknown as MediaStream;
  return { getUserMedia, stream, stop };
}
function renderCamera(onCapture = vi.fn(), onClose = vi.fn()) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(createElement(QueryClientProvider, { client }, createElement(CoverCameraDialog, { onCapture, onClose })));
}
it("captures a cover, releases the camera on preview, and saves only after confirmation", async () => {
  const { getUserMedia, stream, stop } = cameraSetup();
  getUserMedia.mockResolvedValue(stream);
  const onCapture = vi.fn(); const onClose = vi.fn();
  const context = { drawImage: vi.fn() };
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(context as unknown as CanvasRenderingContext2D);
  vi.spyOn(HTMLCanvasElement.prototype, "toDataURL").mockReturnValue(data);
  renderCamera(onCapture, onClose);
  await waitFor(() => expect(screen.getByRole("button", { name: "Take photo" })).toBeEnabled());
  expect(getUserMedia).toHaveBeenCalledWith(expect.objectContaining({ audio: false }));
  const video = document.querySelector("video")!;
  expect(video.srcObject).toBe(stream);
  Object.defineProperty(video, "videoWidth", { value: 1600 });
  Object.defineProperty(video, "videoHeight", { value: 1200 });
  fireEvent.click(screen.getByRole("button", { name: "Take photo" }));
  await screen.findByRole("img", { name: "Cover preview" });
  expect(stop).toHaveBeenCalledTimes(1);
  expect(onCapture).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Retake photo" }));
  await waitFor(() => expect(document.querySelector("video")?.srcObject).toBe(stream));
  const retake = document.querySelector("video")!;
  Object.defineProperty(retake, "videoWidth", { value: 1600 });
  Object.defineProperty(retake, "videoHeight", { value: 1200 });
  await waitFor(() => expect(screen.getByRole("button", { name: "Take photo" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "Take photo" }));
  await screen.findByRole("img", { name: "Cover preview" });
  expect(getUserMedia).toHaveBeenCalledTimes(2);
  expect(stop).toHaveBeenCalledTimes(2);
  fireEvent.click(screen.getByRole("button", { name: "Save photo" }));
  expect(onCapture).toHaveBeenCalledWith(data);
  expect(onClose).toHaveBeenCalledTimes(1);
});
it("releases a camera permission request that completes after the dialog closes", async () => {
  let resolve!: (value: MediaStream) => void;
  const { getUserMedia, stream, stop } = cameraSetup();
  getUserMedia.mockImplementation(() => new Promise((done) => { resolve = done; }));
  const view = renderCamera();
  view.unmount();
  await act(async () => resolve(stream));
  expect(stop).toHaveBeenCalledTimes(1);
});
it("explains denied camera access and allows retry", async () => {
  const { getUserMedia } = cameraSetup();
  getUserMedia.mockRejectedValue(new DOMException("Denied", "NotAllowedError"));
  renderCamera();
  expect(await screen.findByRole("status")).toHaveTextContent("Camera access is blocked");
  fireEvent.click(screen.getByRole("button", { name: "Retry camera" }));
  await waitFor(() => expect(getUserMedia).toHaveBeenCalledTimes(2));
});
