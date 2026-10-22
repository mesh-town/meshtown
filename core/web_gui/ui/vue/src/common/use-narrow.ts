import { onUnmounted, ref, type Ref } from "vue";

export const NARROW_QUERY = "(max-width: 720px)";

/** True while the viewport matches the narrow-screen query; follows resizes. */
export function useNarrow(query: string = NARROW_QUERY): Ref<boolean> {
  const media = typeof matchMedia === "function" ? matchMedia(query) : null;
  const narrow = ref(media !== null && media.matches);
  const onChange = (ev: MediaQueryListEvent): void => {
    narrow.value = ev.matches;
  };
  media?.addEventListener?.("change", onChange);
  onUnmounted(() => media?.removeEventListener?.("change", onChange));
  return narrow;
}
