<script setup lang="ts">
import type { Capture } from "../captures";
import CapturePreview from "./CapturePreview.vue";

const { capture, enhanced } = defineProps<{
  capture: Capture;
  enhanced: boolean;
}>();
const emit = defineEmits<{
  (event: "expand", capture: Capture, trigger: HTMLElement): void;
}>();
const expand = (event: MouseEvent) => {
  if (event.currentTarget instanceof HTMLElement)
    emit("expand", capture, event.currentTarget);
};
</script>

<template>
  <figure :id="`frame-${capture.id}`" class="frame" :data-tab="capture.label">
    <div class="capture-preview">
      <CapturePreview
        :html="capture.html"
        :label="`Terminal capture: ${capture.label}`"
        :fit="enhanced"
      />
      <button
        v-if="enhanced"
        type="button"
        class="capture-open"
        :aria-label="`Expand ${capture.label} capture`"
        aria-haspopup="dialog"
        :title="`Open ${capture.label} capture at full size`"
        @click="expand"
      />
    </div>
    <figcaption class="frame-cap">
      <code>{{ capture.command }}</code>
      <p v-if="capture.id === 'read'">
        Opens in rendered mode. The arrow keys select a block. The Comments
        panel starts empty.
      </p>
      <p v-else-if="capture.id === 'mark'">
        Write the comment and press Enter. It saves to a sidecar file, so the
        Markdown stays unchanged.
      </p>
      <p v-else-if="capture.id === 'send'">
        Each open comment carries its location, the quoted text, and a stable
        ID.
      </p>
    </figcaption>
  </figure>
</template>
