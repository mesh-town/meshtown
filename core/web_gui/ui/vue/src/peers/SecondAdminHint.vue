<script setup lang="ts">
// The hint above the member list while this computer is the only admin of the network: name a second admin.
import { SECOND_ADMIN_HINT } from "../members/members-texts.ts";
import { SECOND_ADMIN_HINT_ID, secondAdminHintShown } from "./second-admin-hint.ts";

const props = defineProps<{ onlyAdmin: boolean }>();
</script>

<template>
  <p v-if="secondAdminHintShown(props.onlyAdmin)" :id="SECOND_ADMIN_HINT_ID" class="hint second-admin-hint" role="status">{{ SECOND_ADMIN_HINT }}</p>
</template>
