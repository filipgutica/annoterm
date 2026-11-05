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
      { label: "Commands", href: "#commands-title" },
      { label: "Keys", href: "#keys-title" },
      { label: "Good to know", href: "#limits-title" },
    ],
  },
];
const commands = [
  {
    command: "annoterm README.md",
    description: "Open a Markdown file to read and comment on it.",
  },
  {
    command: "annoterm copy-feedback README.md",
    description: "Copy the feedback again if something else replaced your clipboard.",
  },
  { command: "annoterm export README.md --output feedback.md" },
];
const keys = [
  { command: "a", description: "Comment on the selected block." },
  {
    command: "Ctrl+R",
    description:
      "Switch between rendered mode and raw mode, where you edit the Markdown source.",
  },
  {
    command: "Ctrl+K",
    description: "Comment on the text selected in raw mode, or on the selected block.",
  },
  { command: "Ctrl+S", description: "Save your edits in raw mode." },
  { command: "q", description: "Quit from rendered mode." },
];
</script>

<template>
  <a class="fg-site-skip" href="#main">Skip to content</a>
  <div class="fg-site-docs" :data-enhanced="enhanced">
    <UiSiteHeader project="annoterm" :links="headerLinks" />
    <UiSiteNav label="On this page" :groups="navigation" />
    <main id="main">
      <section id="install" class="intro" aria-labelledby="title">
        <h1 id="title">Review Markdown in the terminal.</h1>
        <p class="lede">
          annoterm shows a Markdown file in your terminal and lets you comment
          on any paragraph, list, table, or selected text. It turns your open
          comments into feedback: a prompt you paste into Codex, Claude Code,
          or another coding agent so it knows exactly what to change.
        </p>
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
          Homebrew builds annoterm from source and installs Rust for the build.
          <a href="https://brew.sh/">Install Homebrew</a> if you don't have
          it.
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
      </section>

      <section class="stage" aria-label="annoterm in use">
        <p class="fg-site-meta">
          These captures show a demo document in annoterm 0.2.0.
        </p>
        <UiCapture :items="captures" label="Steps">
          <template #caption="{ item }">
            <p v-if="item.id === 'read'">
              annoterm opens the file in rendered mode, which shows the
              formatted document. Press Up or Down to select a block, such as
              a paragraph or a table. The Comments panel on the right is empty
              until you add a comment.
            </p>
            <p v-else-if="item.id === 'mark'">
              Press <kbd>a</kbd>, write your comment, and press Enter. annoterm
              saves the comment in its own file, so your Markdown file stays
              unchanged.
            </p>
            <p v-else-if="item.id === 'send'">
              Each open comment becomes one section of the feedback, with its
              line numbers, the text you selected, and an ID. The agent can use
              the IDs to say which comment each change answers.
            </p>
          </template>
        </UiCapture>
        <UiCard class="legend" title="On screen">
          <dl class="fg-site-legend">
            <div>
              <dt>●</dt>
              <dd>This block has a comment.</dd>
            </div>
            <div>
              <dt>◌</dt>
              <dd>
                The text changed after you commented, so annoterm shows the
                comment at the closest location it can find.
              </dd>
            </div>
            <div>
              <dt>Open</dt>
              <dd>
                An open comment goes into the feedback. A resolved comment stays
                saved but is left out.
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
          annoterm copies the feedback to your clipboard each time you change a
          comment, and again when you quit with open comments.
        </p>
        <UiCommandList :items="commands">
          <template #description="{ item }">
            <template v-if="item.description">{{
              item.description
            }}</template>
            <template v-else
              >Write the feedback to a file. Add <code>--force</code> to replace
              an existing file.</template
            >
          </template>
        </UiCommandList>
      </section>

      <section aria-labelledby="keys-title">
        <h2 id="keys-title">Keys</h2>
        <p class="section-intro">
          Press <kbd>?</kbd> in rendered mode or the Comments panel to see
          every shortcut.
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
            annoterm keeps each document's comments in a JSON file under
            <code>~/.annoterm</code>. When the document changes, each comment
            moves to follow its text. It needs no account or server, and
            it never sends your documents or comments over the network.
          </dd>
          <dt>External file changes</dt>
          <dd>
            If another program changes the file while you have it open,
            annoterm refuses to save over that change and shows an error. It
            has no merge view to combine the two versions.
          </dd>
          <dt>Linux clipboard</dt>
          <dd>
            If copying fails on Linux, install <code>wl-copy</code> for Wayland
            or <code>xclip</code> for X11. Without either, annoterm falls back
            to OSC 52, a terminal escape code that some terminals and SSH
            sessions block.
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
