/** getUserMedia is the permission request; Permissions.query is not supported everywhere. */
export function cameraErrorMessage(error: unknown, desktop: boolean): string {
  const name = error && typeof error === "object" && "name" in error ? error.name : "";
  switch (name) {
    case "NotAllowedError":
    case "PermissionDeniedError":
    case "SecurityError":
      return desktop
        ? "Camera access is blocked. Allow ShelfLedger in your system's camera privacy settings " +
            "(on macOS: System Settings → Privacy & Security → Camera), then retry. " +
            "You may need to restart the app after changing system permissions."
        : "Camera access is blocked. Allow the camera in this site's browser permissions and " +
            "your system's camera privacy settings, then retry.";
    case "NotFoundError":
    case "DevicesNotFoundError":
      return "No camera was found. Connect a camera and retry, or scan from a photo.";
    case "NotReadableError":
    case "TrackStartError":
      return "The camera could not be started. Close other apps using it and retry, " +
        "or scan from a photo.";
    default:
      return "The camera could not be started. Check your camera and its permissions, " +
        "then retry or scan from a photo.";
  }
}
