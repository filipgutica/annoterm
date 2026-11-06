<script setup lang="ts">
import { onMounted, ref } from "vue";
import { UiCard, UiCodeBlock } from "@filipgutica/ui";
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

const headerLinks = [
  { label: "GitHub", href: "https://github.com/filipgutica/annoterm" },
  { label: "Releases", href: "https://github.com/filipgutica/annoterm/releases" },
];
const navigation = [
  {
    items: [
      { label: "Install", href: "#install" },
      { label: "Start a review", href: "#review-title" },
      { label: "Commands", href: "#commands-title" },
      { label: "Keys", href: "#keys-title" },
      { label: "Good to know", href: "#limits-title" },
    ],
  },
];
const commands = [
  {
    command: "annoterm copy-feedback README.md",
    description: "Copy the feedback again after another app replaces your clipboard.",
  },
  { command: "annoterm export README.md --output feedback.md" },
  {
    command: "annoterm --annotations review.json README.md",
    description:
      "Choose a comment file, for example one you keep in the repository. Use --annotations with copy-feedback and export too.",
  },
];
const keys = [
  {
    command: "Ctrl+R",
    description: "Switch between rendered mode and raw mode to edit the Markdown source.",
  },
  {
    command: "Shift + arrow keys",
    description: "Select text in raw mode.",
  },
  {
    command: "Ctrl+K",
    description: "Start a comment on the raw selection or the selected rendered block.",
  },
  {
    command: "Ctrl+W",
    description: "Focus the Comments panel. Use Up or Down to select a comment.",
  },
  {
    command: "x",
    description: "Resolve or reopen the selected comment in rendered mode or the Comments panel.",
  },
  { command: "Esc", description: "Return from the Comments panel to the document." },
  { command: "Ctrl+S", description: "Save Markdown edits." },
  { command: "Ctrl+Q", description: "Quit from either document mode." },
];
</script>

<template>
  <a class="fg-site-skip" href="#main">Skip to content</a>
  <div class="fg-site-docs" :data-enhanced="enhanced">
    <UiSiteHeader project="annoterm" :links="headerLinks" />
    <UiSiteNav label="On this page" :groups="navigation" />
    <main id="main">
      <section id="install" class="fg-site-hero" aria-labelledby="title">
        <h1 id="title">Review Markdown in the terminal.</h1>
        <div class="fg-site-hero__lede">
          <p class="lede">
            annoterm is a terminal app for reviewing Markdown plans and specs.
            Attach comments to rendered blocks or selected source text.
            It builds a prompt with your comments and their locations for your coding agent.
          </p>
        </div>
        <div class="fg-site-hero__aside">
          <div class="install">
            <p class="fg-site-meta">Install with Homebrew</p>
            <UiCodeBlock
              code="brew install filipgutica/tap/annoterm"
              language="bash"
              variant="compact"
              :copyable="enhanced"
              :wrap="true"
            />
          </div>
          <p class="fg-site-meta">
            Homebrew builds annoterm from source with Rust as a build dependency.
            <a href="https://brew.sh/">Install Homebrew</a> if you don't have it.
          </p>
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
        </div>
      </section>

      <section aria-labelledby="review-title">
        <h2 id="review-title">From review to agent</h2>
        <p class="section-intro">
          Use your own Markdown file in place of <code>README.md</code>.
        </p>
        <ol class="fg-site-steps">
          <li>
            <h3>Open the file</h3>
            <p>
              Run <code>annoterm README.md</code>. The file opens in rendered
              mode. Press Up or Down to select a block.
            </p>
          </li>
          <li>
            <h3>Add your comments</h3>
            <p>
              Press <kbd>a</kbd> to comment on the selected block. Write your
              comment. Press Enter to save it.
            </p>
          </li>
          <li>
            <h3>Hand off the feedback</h3>
            <p>
              Press <kbd>q</kbd> to quit from the rendered document. If open
              comments remain, annoterm tries to copy the feedback to your
              clipboard. Check the result in your terminal.
            </p>
            <p class="fg-site-steps__result">
              Paste the feedback into Codex, Claude Code, or another coding agent.
              If copying fails, run
              <code>annoterm export README.md --output feedback.md</code>.
              Then paste the contents of <code>feedback.md</code> into your agent.
            </p>
          </li>
        </ol>
      </section>

      <section class="stage" aria-label="annoterm in use">
        <p class="fg-site-meta">
          These captures show a demo document in annoterm 0.2.0.
        </p>
        <UiCapture :items="captures" label="Steps">
          <template #caption="{ item }">
            <p v-if="item.id === 'read'">
              Demo: the marker beside the paragraph shows the selected block.
            </p>
            <p v-else-if="item.id === 'mark'">
              Demo: the dot marks the commented block. The Comments panel lists the
              comment as open, so it goes into the feedback.
            </p>
            <p v-else-if="item.id === 'send'">
              Demo: each open comment becomes one section, with its lines, state,
              selected text, and ID. The agent can use the ID to say which comment
              each change answers.
            </p>
          </template>
        </UiCapture>
        <UiCard class="legend" title="On screen">
          <dl class="fg-site-legend">
            <div>
              <dt>●</dt>
              <dd>Comment anchored to its current text.</dd>
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
                Only open comments enter the feedback. Resolved comments stay saved.
              </dd>
            </div>
            <div>
              <dt>Bottom bar</dt>
              <dd>Shows the shortcuts for the pane you are using.</dd>
            </div>
          </dl>
        </UiCard>
      </section>

      <section aria-labelledby="commands-title">
        <h2 id="commands-title">Commands</h2>
        <p class="section-intro">
          annoterm tries to copy feedback after each comment change.
          Use these commands to copy it again or export a file.
        </p>
        <UiCommandList :items="commands">
          <template #description="{ item }">
            <template v-if="item.description">{{
              item.description
            }}</template>
            <template v-else
              >Write feedback to a file. Export refuses to replace an existing file
              unless you add <code>--force</code>.</template
            >
          </template>
        </UiCommandList>
      </section>

      <section aria-labelledby="keys-title">
        <h2 id="keys-title">Keys</h2>
        <p class="section-intro">
          To open shortcut help, press <kbd>?</kbd> in rendered mode or the Comments panel.
          In raw mode or while writing a comment, press F1.
        </p>
        <UiCommandList :items="keys" :copyable="false" command-header="Key">
          <template #command="{ item }"><kbd>{{ item.command }}</kbd></template>
        </UiCommandList>
      </section>

      <section aria-labelledby="limits-title">
        <h2 id="limits-title">Good to know</h2>
        <dl class="fg-site-definitions">
          <dt>Comment files</dt>
          <dd>
            annoterm stores comments separately from the Markdown file in a JSON sidecar.
            By default, these comment files live under <code>~/.annoterm</code>.
            When the document changes, annoterm tries to keep each comment on its text.
          </dd>
          <dt>Saving Markdown</dt>
          <dd>
            Starting or submitting a comment saves unsaved Markdown edits first.
            Otherwise, commenting leaves the Markdown file unchanged.
          </dd>
          <dt>Quitting with unsaved edits</dt>
          <dd>
            With unsaved Markdown edits, the first quit press warns you.
            Press the quit key again to discard those edits.
          </dd>
          <dt>External file changes</dt>
          <dd>
            If another program changes the Markdown file, annoterm refuses to save over that change.
            The status line shows an error.
            annoterm has no merge view to combine the two versions.
          </dd>
          <dt>Linux clipboard</dt>
          <dd>
            For Wayland, install <code>wl-copy</code>.
            For X11, install <code>xclip</code>.
            If neither works, annoterm falls back to OSC 52.
            Some terminals and SSH sessions block it.
            OSC 52 cannot confirm delivery, and annoterm sends at most 100,000 bytes this way.
          </dd>
          <dt>Local review</dt>
          <dd>
            annoterm needs no account or server.
            It never sends your documents or comments over the network.
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
    <footer class="fg-site-footer">
      <a href="https://github.com/filipgutica">Built by Filip Gutica</a>
      <nav aria-label="Project links">
        <a href="https://github.com/filipgutica/homebrew-tap">Homebrew tap</a>
        <a href="https://github.com/filipgutica/annoterm/issues"
          >Report an issue</a
        >
        <a href="https://github.com/filipgutica/annoterm/releases">Releases</a>
      </nav>
    </footer>
  </div>
</template>
