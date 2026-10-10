<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, useTemplateRef } from 'vue';
import { UiButton, UiDrawer, useActiveSection } from '@filipgutica/ui';
import NavigationLinks from './NavigationLinks.vue';

const enhanced = ref(false);
const mobile = ref(false);
const open = ref(false);
const header = useTemplateRef<HTMLElement>('header');
const activeId = useActiveSection({
  targetIds: ['install', 'commands-title', 'keys-title', 'limits-title'],
});
let media: MediaQueryList | undefined;
let destination: HTMLElement | null = null;

const syncBreakpoint = () => {
  const leavingFocusedRail =
    media?.matches &&
    !mobile.value &&
    header.value?.querySelector('.site-menu')?.contains(document.activeElement);
  const leavingFocusedTrigger =
    media?.matches === false &&
    mobile.value &&
    !open.value &&
    header.value?.querySelector('button.menu-toggle') === document.activeElement;
  mobile.value = media?.matches === true;
  if (!mobile.value) open.value = false;
  if (leavingFocusedRail) {
    void nextTick(() =>
      header.value
        ?.querySelector<HTMLElement>('button.menu-toggle')
        ?.focus({ preventScroll: true }),
    );
  } else if (leavingFocusedTrigger) {
    void nextTick(() =>
      header.value?.querySelector<HTMLElement>('.section-nav a')?.focus({ preventScroll: true }),
    );
  }
};
const navigate = (href: string, event: MouseEvent) => {
  if (
    !open.value ||
    event.button !== 0 ||
    event.metaKey ||
    event.ctrlKey ||
    event.altKey ||
    event.shiftKey
  )
    return;
  destination = href.startsWith('#') ? document.getElementById(href.slice(1)) : null;
  open.value = false;
};
const closeAutoFocus = (event: Event) => {
  if (destination) {
    event.preventDefault();
    destination.tabIndex = -1;
    destination.focus({ preventScroll: true });
    destination = null;
  } else if (!mobile.value) {
    // The drawer trigger disappears when crossing into the desktop rail.
    event.preventDefault();
    void nextTick(() =>
      header.value?.querySelector<HTMLElement>('.section-nav a')?.focus({ preventScroll: true }),
    );
  }
};
onMounted(() => {
  media = matchMedia('(max-width: 800px)');
  syncBreakpoint();
  enhanced.value = true;
  media.addEventListener('change', syncBreakpoint);
});
onUnmounted(() => media?.removeEventListener('change', syncBreakpoint));
</script>

<template>
  <header ref="header" class="site-header" :data-navigation-enhanced="enhanced">
    <details v-if="!enhanced || !mobile" class="site-menu" :open="enhanced && !mobile">
      <summary class="menu-toggle" aria-controls="site-menu-links">Menu</summary>
      <NavigationLinks :active-id="activeId" />
    </details>
    <UiDrawer v-else v-model:open="open" title="Navigation" @close-auto-focus="closeAutoFocus">
      <template #trigger
        ><UiButton variant="ghost" size="lg" class="menu-toggle">Menu</UiButton></template
      >
      <NavigationLinks
        :active-id="activeId"
        @navigate="navigate"
      />
    </UiDrawer>
  </header>
</template>
