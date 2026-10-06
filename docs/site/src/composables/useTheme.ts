import { onMounted, onUnmounted, ref } from 'vue';

export type ThemeChoice = 'system' | 'light' | 'dark';
const isThemeChoice = (value: string | null): value is ThemeChoice =>
  value === 'system' || value === 'light' || value === 'dark';

export const useTheme = () => {
  const choice = ref<ThemeChoice>('system');
  let systemDark: MediaQueryList | undefined;
  const apply = () => {
    document.documentElement.classList.toggle(
      'dark',
      choice.value === 'dark' || (choice.value === 'system' && systemDark?.matches === true),
    );
    if (choice.value === 'system') delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = choice.value;
  };
  const choose = (value: ThemeChoice) => {
    choice.value = value;
    apply();
    try {
      localStorage.setItem('tool-site-theme', value);
    } catch {
      /* The choice works for this visit. */
    }
  };
  const receiveTheme = (event: StorageEvent) => {
    if (event.key !== 'tool-site-theme') return;
    choice.value = isThemeChoice(event.newValue) ? event.newValue : 'system';
    apply();
  };
  onMounted(() => {
    systemDark = matchMedia('(prefers-color-scheme: dark)');
    try {
      const saved = localStorage.getItem('tool-site-theme');
      if (isThemeChoice(saved)) choice.value = saved;
    } catch {
      /* The system preference works without storage. */
    }
    apply();
    systemDark.addEventListener('change', apply);
    window.addEventListener('storage', receiveTheme);
  });
  onUnmounted(() => {
    systemDark?.removeEventListener('change', apply);
    window.removeEventListener('storage', receiveTheme);
  });
  return { choice, choose };
};
