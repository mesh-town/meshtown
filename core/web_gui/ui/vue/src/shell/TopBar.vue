<script setup lang="ts">
// The column header: the status line with a colored dot. `status` — `{ level, text }`; `votes` — how many join requests
// wait for this admin (a link after the text, none for 0); `href` — where it leads (the Members tab by default).
import { useNarrow } from "../common/use-narrow.ts";
import { stripVoteShortText, stripVoteText } from "../members/members-texts.ts";
import type { StatusLine } from "./status-line.ts";

withDefaults(defineProps<{ status: StatusLine; votes?: number; href?: string }>(), { votes: 0, href: "#peers" });
const narrow = useNarrow();
const badgeText = (n: number): string => (narrow.value ? stripVoteShortText(n) : stripVoteText(n));
</script>

<template>
  <header class="topbar">
    <p id="status-line" class="status" :role="status.alert ? 'alert' : 'status'">
      <span class="dot" :class="'dot-' + status.level" aria-hidden="true"></span><span class="status-text" :title="status.text">{{ status.text }}</span><a v-if="votes > 0" id="strip-vote" :href="href">{{ badgeText(votes) }}</a>
    </p>
  </header>
</template>
