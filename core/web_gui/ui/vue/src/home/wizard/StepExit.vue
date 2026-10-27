<script setup lang="ts">
// Wizard exit screen: the member to route blocked sites through, and (in a browser) this computer's own switch.
import { inject } from "vue";
import { STORE_KEY } from "../../core/store.ts";
import ExitCard from "../ExitCard.vue";
import ThisComputerCard from "../ThisComputerCard.vue";
import { HOME_COMPUTER_TITLE } from "../home-card-texts.ts";
import { useHomeContext } from "../use-home.ts";
import { WIZ_EXIT_DIRECT_WARN } from "./wizard-texts.ts";

defineProps<{ inApp: boolean }>();
const { form } = useHomeContext(inject(STORE_KEY)!);
</script>

<template>
  <ExitCard bare />
  <template v-if="!inApp">
    <h4>{{ HOME_COMPUTER_TITLE }}</h4>
    <ThisComputerCard bare />
  </template>
  <p v-if="form.exitChoice === ''" class="hint" id="exit-direct-warn">{{ WIZ_EXIT_DIRECT_WARN }}</p>
</template>
