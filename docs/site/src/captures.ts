import read from "./captures/read.html?raw";
import mark from "./captures/mark.html?raw";
import send from "./captures/send.html?raw";

export interface Capture {
  id: string;
  label: string;
  command: string;
  html: string;
}

export const captures: readonly Capture[] = [
  {
    id: "read",
    label: "Read",
    command: "annoterm docs/retry-policy.md",
    html: read,
  },
  { id: "mark", label: "Mark", command: "a, then Enter", html: mark },
  {
    id: "send",
    label: "Send",
    command: "annoterm export docs/retry-policy.md --output feedback.md",
    html: send,
  },
];
