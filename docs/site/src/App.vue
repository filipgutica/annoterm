<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, useTemplateRef } from "vue";
import { UiButton, UiCodeBlock, UiDialog, UiTabs } from "@filipgutica/ui";
import SiteNavigation from "./components/SiteNavigation.vue";
import CaptureFigure from "./components/CaptureFigure.vue";
import CapturePreview from "./components/CapturePreview.vue";
import { captures, type Capture } from "./captures";
import { useScrollReveal } from "./composables/useScrollReveal";

const main = useTemplateRef<HTMLElement>("main");
useScrollReveal(main);

const enhanced = ref(false);
const activeCapture = ref("read");
const captureTabs = captures.map(({ id, label }) => ({ value: id, label }));
const captureOpen = ref(false);
const viewedCapture = ref<Capture>();
let captureTrigger: HTMLElement | undefined;
let firstFrame = 0;
let secondFrame = 0;

const openCapture = (capture: Capture, trigger: HTMLElement) => {
  viewedCapture.value = capture;
  captureTrigger = trigger;
  captureOpen.value = true;
};
const restoreCaptureFocus = (event: Event) => {
  if (!captureTrigger?.isConnected) return;
  event.preventDefault();
  captureTrigger.focus({ preventScroll: true });
};
const currentHashTarget = () => {
  try {
    return document.getElementById(decodeURIComponent(location.hash.slice(1)));
  } catch {
    return null;
  }
};
const alignCurrentHash = async () => {
  await nextTick();
  cancelAnimationFrame(firstFrame);
  cancelAnimationFrame(secondFrame);
  firstFrame = requestAnimationFrame(() => {
    secondFrame = requestAnimationFrame(() =>
      currentHashTarget()?.scrollIntoView({
        behavior: "instant",
        block: "start",
      }),
    );
  });
};
const selectHashCapture = () => {
  const target = currentHashTarget();
  const capture = captures.find(({ id }) => target?.closest(`#frame-${id}`));
  if (!capture || activeCapture.value === capture.id) return false;
  activeCapture.value = capture.id;
  return true;
};
const onHashChange = () => {
  if (selectHashCapture()) void alignCurrentHash();
};
onMounted(() => {
  enhanced.value = true;
  selectHashCapture();
  // Hydration hides inactive captures. Realign a deep link after preview sizing settles.
  if (location.hash) void alignCurrentHash();
  window.addEventListener("hashchange", onHashChange);
  window.addEventListener("popstate", onHashChange);
});
onUnmounted(() => {
  cancelAnimationFrame(firstFrame);
  cancelAnimationFrame(secondFrame);
  window.removeEventListener("hashchange", onHashChange);
  window.removeEventListener("popstate", onHashChange);
});
</script>

<template>
  <a class="skip" href="#main">Skip to content</a>
  <div class="page" :data-enhanced="enhanced">
    <SiteNavigation />
    <header class="page-header">
      <a class="page-brand" href="/annoterm/" aria-label="annoterm home">annoterm</a>
      <nav aria-label="Main navigation">
        <a href="https://github.com/filipgutica/annoterm/blob/main/README.md">Guide</a>
        <a href="https://github.com/filipgutica/annoterm">GitHub</a>
        <a href="https://github.com/filipgutica/annoterm/releases">Releases</a>
      </nav>
    </header>
    <main id="main" ref="main">
      <div>
        <section class="hero" aria-labelledby="title">
          <h1 id="title" class="tagline">Read it. Mark it. Send it back.</h1>
          <p class="lede">
            Review Markdown in the terminal and send comments to your coding agent.
          </p>
          <div id="install" class="install-command">
            <p class="hint">Install with Homebrew</p>
            <UiCodeBlock
              code="brew install filipgutica/tap/annoterm"
              language="bash"
              variant="compact"
              :copyable="enhanced"
              :wrap="true"
            />
          </div>
          <p class="hint">
            Homebrew builds from source and installs Rust.
            <a href="https://brew.sh/">Install Homebrew</a> if needed.
          </p>
          <dl class="facts">
            <div>
              <dt>Runs on</dt>
              <dd>macOS and Linux</dd>
            </div>
            <div>
              <dt>Written in</dt>
              <dd>Rust</dd>
            </div>
            <div>
              <dt>License</dt>
              <dd>MIT</dd>
            </div>
            <div>
              <dt>Source</dt>
              <dd>
                <a href="https://github.com/filipgutica/annoterm"
                  >filipgutica/annoterm</a
                >
              </dd>
            </div>
          </dl>
        </section>
        <section class="stage" aria-label="annoterm in use">
          <p class="stage-label">
            Demo document in annoterm 0.2.0.
          </p>
          <UiTabs v-model="activeCapture" :items="captureTabs" label="Steps">
            <template #panel="{ value }">
              <template v-for="capture in captures" :key="capture.id">
                <CaptureFigure
                  v-if="capture.id === value"
                  :capture="capture"
                  :enhanced="enhanced"
                  @expand="openCapture"
                />
              </template>
            </template>
          </UiTabs>
          <aside class="legend" aria-labelledby="legend-title">
            <h3 id="legend-title">On screen</h3>
            <dl style="--cols: 4">
              <div>
                <dt>●</dt>
                <dd>Comment anchored to this block.</dd>
              </div>
              <div>
                <dt>◌</dt>
                <dd>
                  Outdated comment at an approximate location.
                </dd>
              </div>
              <div>
                <dt>Open</dt>
                <dd>
                  Open comments enter feedback; resolved comments stay only in the sidecar.
                </dd>
              </div>
              <div>
                <dt>Bottom bar</dt>
                <dd>Active pane's shortcuts.</dd>
              </div>
            </dl>
          </aside>
        </section>
      </div>
      <section class="split" aria-labelledby="commands-title">
        <header>
          <h2 id="commands-title">Commands</h2>
          <p>
            Feedback is copied after each comment change and on quit with open comments.
          </p>
        </header>
        <ul class="rows">
          <li class="row">
            <div class="cmd">
              <UiCodeBlock
                variant="compact"
                code="annoterm README.md"
                language="bash"
                :copyable="enhanced"
                :wrap="true"
              />
            </div>
            <p>Open rendered Markdown.</p>
          </li>
          <li class="row">
            <div class="cmd">
              <UiCodeBlock
                variant="compact"
                code="annoterm copy-feedback README.md"
                language="bash"
                :copyable="enhanced"
                :wrap="true"
              />
            </div>
            <p>
              Copy feedback again if your clipboard was replaced.
            </p>
          </li>
          <li class="row">
            <div class="cmd">
              <UiCodeBlock
                variant="compact"
                code="annoterm export README.md --output feedback.md"
                language="bash"
                :copyable="enhanced"
                :wrap="true"
              />
            </div>
            <p>
              Export feedback; <code>--force</code> replaces an existing file.
            </p>
          </li>
        </ul>
      </section>
      <section class="split narrow" aria-labelledby="keys-title">
        <header>
          <h2 id="keys-title">Keys</h2>
          <p><kbd>?</kbd> shows all shortcuts in rendered mode or the Comments panel.</p>
        </header>
        <ul class="rows">
          <li class="row">
            <kbd>a</kbd>
            <p>Comment on the selected block.</p>
          </li>
          <li class="row">
            <kbd>Ctrl+R</kbd>
            <p>Toggle rendered and raw modes.</p>
          </li>
          <li class="row">
            <kbd>Ctrl+K</kbd>
            <p>Comment on a raw selection or rendered block.</p>
          </li>
          <li class="row">
            <kbd>Ctrl+S</kbd>
            <p>Save raw changes.</p>
          </li>
          <li class="row">
            <kbd>q</kbd>
            <p>Quit rendered mode; copy open comments.</p>
          </li>
        </ul>
      </section>
      <section class="split" aria-labelledby="limits-title">
        <header>
          <h2 id="limits-title">Good to know</h2>
        </header>
        <div class="text-rows">
          <p>
            <strong>Comments:</strong> JSON sidecars under <code>~/.annoterm</code>,
            separate from your document. Anchors follow source changes.
          </p>
          <p>
            <strong>External file changes stop saves.</strong> No merge view.
          </p>
          <p>
            <strong>Linux clipboard:</strong> copying may need <code>wl-copy</code> for Wayland
            or <code>xclip</code> for X11. OSC 52 is the fallback.
          </p>
          <nav class="links" aria-label="Documentation">
            <a href="https://github.com/filipgutica/annoterm#readme"
              >Full reference</a
            >
            <a
              href="https://github.com/filipgutica/annoterm/blob/main/docs/terminal-support.md"
              >Terminal support</a
            >
            <a
              href="https://github.com/filipgutica/annoterm/blob/main/docs/annotation-format.md"
              >Annotation format</a
            >
          </nav>
        </div>
      </section>
      <section class="split" aria-labelledby="family-title">
        <header>
          <h2 id="family-title">Also from Filip</h2>
          <p>
            <a href="https://github.com/filipgutica/homebrew-tap"
              >Homebrew tap</a
            >
          </p>
        </header>
        <ul class="rows narrow">
          <li class="row">
            <a href="https://filipgutica.github.io/wtree/"
              ><code>wtree</code></a
            >
            <p>
              Git worktree status and cleanup.
            </p>
          </li>
          <li class="row">
            <a href="https://filipgutica.github.io/devps/"
              ><code>devps</code></a
            >
            <p>
              Local dev server management.
            </p>
          </li>
          <li class="row">
            <a href="https://filipgutica.github.io/t3code/"
              ><code>Workbench</code></a
            >
            <p>
              Tickets and agent threads across repositories.
            </p>
          </li>
        </ul>
      </section>
    </main>
    <footer>
      <a href="https://github.com/filipgutica">Built by Filip Gutica</a>
      <nav aria-label="Project links">
        <a href="https://github.com/filipgutica/annoterm/issues"
          >Report an issue</a
        >
        <a href="https://github.com/filipgutica/annoterm/releases">Releases</a>
        <a href="https://github.com/filipgutica/annoterm/blob/main/LICENSE"
          >MIT license</a
        >
      </nav>
    </footer>
  </div>
  <UiDialog
    v-model:open="captureOpen"
    :title="`${viewedCapture?.label ?? ''} capture`"
    description="Scroll to read the full capture."
    class="capture-viewer"
    @close-auto-focus="restoreCaptureFocus"
  >
    <CapturePreview
      v-if="viewedCapture"
      :html="viewedCapture.html"
      :label="`${viewedCapture.label} capture at full size`"
      :fit="false"
    />
    <template #footer
      ><UiButton variant="secondary" size="lg" @click="captureOpen = false"
        >Close capture</UiButton
      ></template
    >
  </UiDialog>
</template>
