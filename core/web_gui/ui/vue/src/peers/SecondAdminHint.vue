<script setup lang="ts">
// The hint above the member list while this computer is the only admin of the network: name a second admin.
import { secondAdminHintText } from "../members/members-texts.ts";
import { SECOND_ADMIN_HINT_ID, secondAdminHintShown } from "./second-admin-hint.ts";

const props = defineProps<{ onlyAdmin: boolean; closed?: boolean }>();
</script>

<template>
  <p v-if="secondAdminHintShown(props.onlyAdmin, props.closed)" :id="SECOND_ADMIN_HINT_ID" class="hint second-admin-hint" role="status">{{ secondAdminHintText() }}</p>
</template>
