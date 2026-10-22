<script setup lang="ts">
// The tab bar: the network logo and seven tab links (on a narrow screen four tabs and a "More" menu);
// the active one scrolls into the visible part.
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useNarrow } from "../common/use-narrow.ts";
import { NET_NAME } from "../common/words.ts";
import { CHIP_TITLE, chipLabel } from "../members/members-texts.ts";
import { inAppPage } from "../home/browser-setup.ts";
import { sectionViews, splitTabs, viewTab } from "./nav.ts";
import { SHELL_TAB_MORE, SHELL_TAB_UNSAVED } from "./shell-texts.ts";

const inApp = inAppPage();
const views = sectionViews(inApp);
const props = withDefaults(defineProps<{ active?: string; dirty?: boolean; votes?: number }>(), { active: "home", dirty: false, votes: 0 });
const nav = ref<HTMLElement | null>(null);
const moreBox = ref<HTMLElement | null>(null);
const narrow = useNarrow();
const menuOpen = ref(false);

const split = computed(() => (narrow.value ? splitTabs(views) : { main: views, more: [] }));
const activeMore = computed(() => split.value.more.find((v) => v.id === props.active));
const moreLabel = computed(() => (activeMore.value ? activeMore.value.tab : SHELL_TAB_MORE) + " \u25BE");

function reveal() {
  const tab = nav.value && nav.value.querySelector<HTMLElement>(".tab.active");
  if (tab && tab.scrollIntoView) tab.scrollIntoView({ block: "nearest", inline: "nearest" });
}
watch(() => props.active, () => { menuOpen.value = false; reveal(); }, { flush: "post" });
watch(narrow, () => { menuOpen.value = false; });

function closeOutside(ev: Event): void {
  if (menuOpen.value && !(moreBox.value && ev.target instanceof Node && moreBox.value.contains(ev.target))) menuOpen.value = false;
}
onMounted(() => document.addEventListener("click", closeOutside));
onUnmounted(() => document.removeEventListener("click", closeOutside));

const marked = (id: string): boolean => id === "settings" && props.dirty;
</script>

<template>
  <nav id="tabs" ref="nav" role="tablist">
    <div v-if="!inApp && !narrow" class="tabs-brand"><span class="brand-name">{{ NET_NAME }}</span></div>
    <a
      v-for="v in split.main"
      :id="'tab-' + v.id"
      :key="v.id"
      class="tab"
      :class="{ active: active === v.id, 'has-dirty': marked(v.id) }"
      :href="'#' + v.id"
      role="tab"
      :aria-controls="'sec-' + v.id"
      :aria-selected="active === v.id ? 'true' : 'false'"
      :title="marked(v.id) ? SHELL_TAB_UNSAVED : undefined"
    >{{ viewTab(v, narrow) }}<span v-if="v.id === 'peers' && votes > 0" class="chip" :aria-label="chipLabel(votes)" :title="CHIP_TITLE">{{ votes }}</span></a>
    <div v-if="split.more.length > 0" ref="moreBox" class="tab-more">
      <button type="button" class="tab tab-more-btn" :class="{ active: !!activeMore }" aria-haspopup="menu" :aria-expanded="menuOpen ? 'true' : 'false'" @click="menuOpen = !menuOpen">{{ moreLabel }}</button>
      <div v-if="menuOpen" class="tab-menu" role="menu">
        <a v-for="v in split.more" :id="'tab-' + v.id" :key="v.id" class="tab-menu-item" :class="{ active: active === v.id }" :href="'#' + v.id" role="menuitem" @click="menuOpen = false">{{ v.tab }}</a>
      </div>
    </div>
  </nav>
</template>
