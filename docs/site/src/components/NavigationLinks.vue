<script setup lang="ts">
const { activeId } = defineProps<{ activeId: string | undefined }>();
const emit = defineEmits<{
  (event: 'navigate', href: string, click: MouseEvent): void;
}>();
const sections = [
  { id: "install", label: "Install" },
  { id: "commands-title", label: "Commands" },
  { id: "keys-title", label: "Keys" },
  { id: "limits-title", label: "Good to know" },
] as const;
</script>

<template>
  <div class="nav-content" id="site-menu-links">
    <nav class="section-nav" aria-labelledby="page-nav-label">
      <h2 class="nav-label" id="page-nav-label">On this page</h2>
      <a
        v-for="section in sections"
        :key="section.id"
        :href="`#${section.id}`"
        :aria-current="activeId === section.id ? 'location' : undefined"
        @click="emit('navigate', `#${section.id}`, $event)"
        >{{ section.label }}</a
      >
    </nav>
  </div>
</template>
