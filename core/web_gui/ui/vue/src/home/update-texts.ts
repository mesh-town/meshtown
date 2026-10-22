// Texts of the "update available" banner.

export const UPDATE_DOWNLOAD = "Download";
export const UPDATE_DISMISS = "Dismiss";
export const UPDATE_PACKAGES_HINT = "Homebrew: brew upgrade meshtown · OpenWrt: opkg upgrade meshtown";
export const UPDATE_DOWNLOAD_URL = "https://github.com/mesh-town/meshtown-dist#readme";
export const UPDATE_ANDROID_DOWNLOAD_URL = "https://github.com/mesh-town/meshtown-dist/raw/main/android/meshtown.apk";
export const UPDATE_LATEST_API_URL = "https://api.github.com/repos/mesh-town/meshtown/releases/latest";

export function updateAvailableText(latest: string, own: string): string {
  return `Mesh Town ${latest} is available (this node runs ${own}).`;
}
