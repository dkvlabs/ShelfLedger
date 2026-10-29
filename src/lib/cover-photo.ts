export const MAX_COVER_PHOTO_BYTES = 10 * 1024 * 1024;

/** File bytes only; no remote image URLs or external paths cross IPC. */
export function readCoverPhoto(file: File): Promise<string> {
  if (!file.size || file.size > MAX_COVER_PHOTO_BYTES) {
    return Promise.reject(new Error("Choose a JPEG, PNG or WebP image up to 10 MiB."));
  }
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error("The photo could not be read. Try another image."));
    reader.onload = () => {
      const data = String(reader.result);
      // Some document providers omit MIME; Rust validates actual image bytes.
      resolve(data.replace(/^data:(?:application\/octet-stream)?;base64,/, "data:image/jpeg;base64,"));
    };
    reader.readAsDataURL(file);
  });
}
