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
            Review Markdown in the terminal. Your comments become precise
            feedback for Codex, Claude Code, or another coding agent.
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
            Homebrew builds annoterm from source and installs Rust to do it. New
            to Homebrew? <a href="https://brew.sh/">Install it first</a>.
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
            Demo document. Captured from annoterm 0.2.0.
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
                <dd>A comment is anchored to this block.</dd>
              </div>
              <div>
                <dt>◌</dt>
                <dd>
                  The comment is outdated and shown at an approximate spot.
                </dd>
              </div>
              <div>
                <dt>Open</dt>
                <dd>
                  Open comments go into the feedback. Resolved ones stay in the
                  sidecar and are left out.
                </dd>
              </div>
              <div>
                <dt>Bottom bar</dt>
                <dd>Shortcuts for the pane you are in.</dd>
              </div>
            </dl>
          </aside>
        </section>
      </div>
      <section class="split" aria-labelledby="commands-title">
        <header>
          <h2 id="commands-title">Commands</h2>
          <p>
            annoterm copies feedback after each comment change, and again when
            you quit with open comments.
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
            <p>Open a Markdown file in rendered mode.</p>
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
              Copy the feedback again if another app replaced your clipboard.
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
              Write the feedback to a file. Add <code>--force</code> to replace
              an existing file.
            </p>
          </li>
        </ul>
      </section>
      <section class="split narrow" aria-labelledby="keys-title">
        <header>
          <h2 id="keys-title">Keys</h2>
          <p>Press <kbd>?</kbd> in the app for the full list.</p>
        </header>
        <ul class="rows">
          <li class="row">
            <kbd>a</kbd>
            <p>Comment on the selected block.</p>
          </li>
          <li class="row">
            <kbd>Ctrl+R</kbd>
            <p>Switch between rendered Markdown and raw source.</p>
          </li>
          <li class="row">
            <kbd>Ctrl+K</kbd>
            <p>Comment on a raw selection or a rendered block.</p>
          </li>
          <li class="row">
            <kbd>Ctrl+S</kbd>
            <p>Save changes in raw mode.</p>
          </li>
          <li class="row">
            <kbd>q</kbd>
            <p>Quit from rendered mode and copy open comments.</p>
          </li>
        </ul>
      </section>
      <section class="split" aria-labelledby="limits-title">
        <header>
          <h2 id="limits-title">Good to know</h2>
        </header>
        <div class="text-rows">
          <p>
            <strong>Comments stay outside your document.</strong> annoterm
            stores them in a JSON sidecar under <code>~/.annoterm</code> and
            re-anchors them when the source moves.
          </p>
          <p>
            <strong>Saves stop if another process changed the file.</strong>
            There is no merge view.
          </p>
          <p>
            <strong>On Linux, copying can need a helper.</strong> Install
            <code>wl-copy</code> for Wayland or <code>xclip</code> for X11.
            annoterm falls back to OSC 52.
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
            The three terminal tools install from
            <a href="https://github.com/filipgutica/homebrew-tap"
              >one Homebrew tap</a
            >.
          </p>
        </header>
        <ul class="rows narrow">
          <li class="row">
            <a href="https://filipgutica.github.io/wtree/"
              ><code>wtree</code></a
            >
            <p>
              List Git worktrees with age and pull request state, then clean up
              the finished ones.
            </p>
          </li>
          <li class="row">
            <a href="https://filipgutica.github.io/devps/"
              ><code>devps</code></a
            >
            <p>
              Manage local dev servers: see what started each one, jump back to
              it, or stop it.
            </p>
          </li>
          <li class="row">
            <a href="https://filipgutica.github.io/t3code/"
              ><code>Workbench</code></a
            >
            <p>
              Plan across repositories, organize tickets, and start agent
              threads in worktrees.
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
