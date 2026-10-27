<script setup lang="ts">
// One question of the setup wizard answered by pressing one of a few buttons.
defineProps<{ groupId: string; labelId: string; options: { id: string; domId: string; label: string }[]; picked: string | null }>();
defineEmits<{ pick: [id: string] }>();
</script>

<template>
  <div :id="groupId" role="group" :aria-labelledby="labelId">
    <button v-for="o in options" :key="o.id" type="button" class="btn" :id="o.domId" :aria-pressed="picked === o.id" @click="$emit('pick', o.id)">{{ o.label }}</button>
  </div>
</template>
