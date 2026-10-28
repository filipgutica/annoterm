<script setup lang="ts">
import { onMounted, ref } from "vue";
import { UiBadge, UiCard, UiCodeBlock } from "@filipgutica/ui";
import {
  UiCapture,
  UiCommandList,
  UiSiteHeader,
  UiSiteNav,
} from "@filipgutica/ui/site";
import { captures } from "./captures";

const enhanced = ref(false);
onMounted(() => {
  enhanced.value = true;
});

const navigation = [
  {
    items: [
      { label: "Install", href: "#install" },
      { label: "Commands", href: "#commands-title" },
      { label: "Keys", href: "#keys-title" },
      { label: "Good to know", href: "#limits-title" },
    ],
  },
];
const commands = [
  { command: "annoterm README.md", description: "Open rendered Markdown." },
  {
    command: "annoterm copy-feedback README.md",
    description: "Copy feedback again if your clipboard was replaced.",
  },
  { command: "annoterm export README.md --output feedback.md" },
];
const keys = [
  { command: "a", description: "Comment on the selected block." },
  { command: "Ctrl+R", description: "Toggle rendered and raw modes." },
  {
    command: "Ctrl+K",
    description: "Comment on a raw selection or rendered block.",
  },
  { command: "Ctrl+S", description: "Save raw changes." },
  { command: "q", description: "Quit rendered mode; copy open comments." },
];
</script>

<template>
  <a class="fg-site-skip" href="#main">Skip to content</a>
  <div class="page" :data-enhanced="enhanced">
    <UiSiteHeader
      class="page-header"
      project="annoterm"
      :links="[
        {
          label: 'Guide',
          href: 'https://github.com/filipgutica/annoterm/blob/main/README.md',
        },
        { label: 'GitHub', href: 'https://github.com/filipgutica/annoterm' },
        {
          label: 'Releases',
          href: 'https://github.com/filipgutica/annoterm/releases',
        },
      ]"
    />
    <div class="layout">
      <UiSiteNav label="On this page" :groups="navigation" />
      <main id="main">
        <section id="install" class="intro" aria-labelledby="title">
          <h1 id="title">Read it. Mark it. Send it back.</h1>
          <p class="lede">
            Review Markdown in the terminal and send comments to your coding
            agent.
          </p>
          <p class="command-label">Install with Homebrew</p>
          <UiCodeBlock
            code="brew install filipgutica/tap/annoterm"
            language="bash"
            variant="compact"
            :copyable="enhanced"
            :wrap="true"
          />
          <p class="hint">
            Homebrew builds from source and installs Rust.
            <a href="https://brew.sh/">Install Homebrew</a> if needed.
          </p>
        </section>

        <section class="stage" aria-label="annoterm in use">
          <p class="stage-label">Demo document in annoterm 0.2.0.</p>
          <UiCapture :items="captures" label="Steps">
            <template #caption="{ item }">
              <p v-if="item.id === 'read'">
                Opens in rendered mode. The arrow keys select a block. The
                Comments panel starts empty.
              </p>
              <p v-else-if="item.id === 'mark'">
                Write the comment and press Enter. It saves to a sidecar file,
                so the Markdown stays unchanged.
              </p>
              <p v-else-if="item.id === 'send'">
                Each open comment carries its location, the quoted text, and a
                stable ID.
              </p>
            </template>
          </UiCapture>
          <UiCard class="legend" title="On screen">
            <dl>
              <div>
                <dt class="glyph">●</dt>
                <dd>Comment anchored to this block.</dd>
              </div>
              <div>
                <dt class="glyph">◌</dt>
                <dd>Outdated comment at an approximate location.</dd>
              </div>
              <div>
                <dt>Open</dt>
                <dd>
                  Open comments enter feedback; resolved comments stay only in
                  the sidecar.
                </dd>
              </div>
              <div>
                <dt>Bottom bar</dt>
                <dd>Active pane's shortcuts.</dd>
              </div>
            </dl>
          </UiCard>
          <dl class="fg-site-facts">
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
              <dd><UiBadge>MIT</UiBadge></dd>
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

        <section aria-labelledby="commands-title">
          <h2 id="commands-title">Commands</h2>
          <p class="section-intro">
            Feedback is copied after each comment change and on quit with open
            comments.
          </p>
          <UiCommandList :items="commands">
            <template #description="{ item }">
              <template v-if="item.description">{{
                item.description
              }}</template>
              <template v-else
                >Export feedback; <code>--force</code> replaces an existing
                file.</template
              >
            </template>
          </UiCommandList>
        </section>

        <section aria-labelledby="keys-title">
          <h2 id="keys-title">Keys</h2>
          <p class="section-intro">
            <kbd>?</kbd> shows all shortcuts in rendered mode or the Comments
            panel.
          </p>
          <UiCommandList :items="keys" :copyable="false" command-header="Key">
            <template #command="{ item }"><kbd>{{ item.command }}</kbd></template>
          </UiCommandList>
        </section>

        <section aria-labelledby="limits-title">
          <h2 id="limits-title">Good to know</h2>
          <dl class="fg-site-definitions">
            <dt>Comments</dt>
            <dd>
              JSON sidecars under <code>~/.annoterm</code>, separate from your
              document. Anchors follow source changes.
            </dd>
            <dt>External file changes</dt>
            <dd>Stop saves. No merge view.</dd>
            <dt>Linux clipboard</dt>
            <dd>
              Copying may need <code>wl-copy</code> for Wayland or
              <code>xclip</code> for X11. OSC 52 is the fallback.
            </dd>
          </dl>
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
        </section>
      </main>
    </div>
    <footer>
      <a href="https://github.com/filipgutica">Built by Filip Gutica</a>
      <nav aria-label="Project links">
        <a href="https://github.com/filipgutica/homebrew-tap">Homebrew tap</a>
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
</template>
