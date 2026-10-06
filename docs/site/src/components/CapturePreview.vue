<script setup lang="ts">
import { onMounted, onUnmounted, ref, useTemplateRef, watch } from 'vue';

const { html, label, fit } = defineProps<{ html: string; label: string; fit: boolean }>();
const container = useTemplateRef<HTMLElement>('container');
const scale = ref(1);
const height = ref<number>();
let observer: ResizeObserver | undefined;

const measure = () => {
  const wrap = container.value;
  const terminal = wrap?.querySelector<HTMLElement>('.term');
  if (!fit) {
    height.value = undefined;
    scale.value = 1;
    return;
  }
  if (!wrap?.clientWidth || !terminal?.offsetWidth) return;
  scale.value = Math.min(1, wrap.clientWidth / terminal.offsetWidth);
  height.value =
    Math.ceil(terminal.offsetHeight * scale.value) + wrap.offsetHeight - wrap.clientHeight;
};
watch(() => fit, measure, { flush: 'post' });
onMounted(() => {
  observer = new ResizeObserver(measure);
  if (container.value) observer.observe(container.value);
  measure();
});
onUnmounted(() => observer?.disconnect());
</script>

<template>
  <!-- HTML is limited to immutable terminal artifacts checked into src/captures. -->
  <div
    ref="container"
    class="term-wrap"
    role="region"
    :aria-label="label"
    :tabindex="fit ? undefined : 0"
    :data-preview="fit || undefined"
    :style="{ height: height === undefined ? undefined : `${height}px`, '--capture-scale': scale }"
    v-html="html"
  />
</template>
