<script setup lang="ts">
// The "⋯" menu of one member row: a toggle and a list of items; reports the chosen enabled item with `pick(key)`.
// Does not know what an item does. Keyboard: arrows / Home / End walk the items, Esc and Tab close the menu and
// return the focus to the toggle; a click anywhere else closes it. A disabled item is reachable but does nothing.
import { nextTick, onMounted, onUnmounted, ref } from "vue";
import { moreActionsLabel } from "./members-texts.ts";
import type { MenuItem } from "./members-types.ts";

const props = defineProps<{ id: string; name: string; items: MenuItem[] }>();
const emit = defineEmits<{ (e: "pick", key: string): void }>();

const root = ref<HTMLElement | null>(null);
const toggle = ref<HTMLButtonElement | null>(null);
const open = ref(false);

const entries = (): HTMLElement[] => Array.from(root.value?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? []);

async function show(): Promise<void> {
  open.value = true;
  await nextTick();
  entries()[0]?.focus();
}

function close(): void {
  open.value = false;
  toggle.value?.focus();
}

const onToggle = (): void => (open.value ? close() : void show());

function choose(item: MenuItem): void {
  if (item.disabled) return;
  close();
  emit("pick", item.key);
}

function move(step: number): void {
  const list = entries();
  const at = list.indexOf(document.activeElement as HTMLElement);
  list[(at + step + list.length) % list.length]?.focus();
}

const KEYS: Record<string, () => void> = {
  ArrowDown: () => move(1),
  ArrowUp: () => move(-1),
  Home: () => entries()[0]?.focus(),
  End: () => entries().at(-1)?.focus(),
  Escape: close,
  Tab: close,
};

function onKey(ev: KeyboardEvent): void {
  const act = KEYS[ev.key];
  if (!act) return;
  if (ev.key !== "Tab") ev.preventDefault();
  act();
}

// A click outside this menu (another member's toggle included) closes it.
const onDocClick = (ev: MouseEvent): void => {
  if (open.value && root.value && !root.value.contains(ev.target as Node)) open.value = false;
};
onMounted(() => document.addEventListener("click", onDocClick));
onUnmounted(() => document.removeEventListener("click", onDocClick));
</script>

<template>
  <span ref="root" class="member-menu">
    <button ref="toggle" type="button" class="member-menu-btn" :data-member-menu="id" aria-haspopup="menu" :aria-expanded="open ? 'true' : 'false'" :aria-label="moreActionsLabel(name)" @click="onToggle">⋯</button>
    <ul v-if="open" class="menu" role="menu" @keydown="onKey">
      <li v-for="it in props.items" :key="it.key" role="none">
        <button type="button" class="mi" :class="{ danger: it.danger }" role="menuitem" :data-member-item="it.key" :aria-disabled="it.disabled ? 'true' : undefined" :title="it.title" @click="choose(it)">{{ it.label }}</button>
      </li>
    </ul>
  </span>
</template>
