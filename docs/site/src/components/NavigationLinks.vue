<script setup lang="ts">
import { UiButton } from "@filipgutica/ui";
import type { ThemeChoice } from "../composables/useTheme";

const { activeId, theme, enhanced } = defineProps<{
  activeId: string | undefined;
  theme: ThemeChoice;
  enhanced: boolean;
}>();
const emit = defineEmits<{
  (event: "theme", value: ThemeChoice): void;
  (event: "navigate", href: string, click: MouseEvent): void;
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
    <nav class="family" aria-labelledby="project-nav-label">
      <h2 class="nav-label" id="project-nav-label">Projects</h2>
      <a
        href="https://filipgutica.github.io/annoterm/"
        aria-current="page"
        @click="
          emit('navigate', 'https://filipgutica.github.io/annoterm/', $event)
        "
        >annoterm</a
      >
      <a
        href="https://filipgutica.github.io/wtree/"
        @click="
          emit('navigate', 'https://filipgutica.github.io/wtree/', $event)
        "
        >wtree</a
      >
      <a
        href="https://filipgutica.github.io/devps/"
        @click="
          emit('navigate', 'https://filipgutica.github.io/devps/', $event)
        "
        >devps</a
      >
      <a
        href="https://filipgutica.github.io/t3code/"
        @click="
          emit('navigate', 'https://filipgutica.github.io/t3code/', $event)
        "
        >workbench</a
      >
      <a
        href="https://filipgutica.github.io/ui/"
        @click="emit('navigate', 'https://filipgutica.github.io/ui/', $event)"
        >Vue UI</a
      >
    </nav>
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
    <div v-if="enhanced" class="appearance">
      <span class="appearance-label">Appearance</span>
      <div class="theme-switch" role="group" aria-label="Color theme">
        <UiButton
          v-for="value in ['system', 'light', 'dark'] as const"
          :key="value"
          variant="ghost"
          size="lg"
          class="theme-choice"
          :data-theme-choice="value"
          :aria-label="`${value[0].toUpperCase() + value.slice(1)} theme`"
          :title="`${value[0].toUpperCase() + value.slice(1)} theme`"
          :aria-pressed="theme === value"
          @click="emit('theme', value)"
        >
          <svg
            v-if="value === 'system'"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            aria-hidden="true"
          >
            <rect x="3" y="4" width="18" height="13" rx="2" />
            <path d="M8 21h8m-4-4v4" />
          </svg>
          <svg
            v-else-if="value === 'light'"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            aria-hidden="true"
          >
            <circle cx="12" cy="12" r="4" />
            <path
              d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5"
            />
          </svg>
          <svg
            v-else
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            aria-hidden="true"
          >
            <path d="M20.5 13.5A9 9 0 0 1 10.5 3a9 9 0 1 0 10 10.5Z" />
          </svg>
        </UiButton>
      </div>
      <span class="theme-choice-label">{{
        theme[0].toUpperCase() + theme.slice(1)
      }}</span>
    </div>
  </div>
</template>
