import { onMounted, onUnmounted, type Ref } from 'vue';

export const useScrollReveal = (main: Readonly<Ref<HTMLElement | null>>) => {
  let observer: IntersectionObserver | undefined;
  let motion: MediaQueryList | undefined;
  let sections: HTMLElement[] = [];
  const revealAll = () => {
    if (!motion?.matches) return;
    sections.forEach((section) => section.classList.remove('reveal-pending'));
    observer?.disconnect();
  };
  onMounted(() => {
    motion = matchMedia('(prefers-reduced-motion: reduce)');
    if (!main.value || !('IntersectionObserver' in window) || motion.matches) return;
    sections = [...main.value.querySelectorAll<HTMLElement>('.stage, :scope > .split')];
    observer = new IntersectionObserver((entries) => {
      entries.forEach(({ target, isIntersecting }) => {
        if (!isIntersecting) return;
        target.classList.remove('reveal-pending');
        observer?.unobserve(target);
      });
    });
    sections.forEach((section) => {
      if (section.getBoundingClientRect().top < innerHeight) return;
      section.classList.add('scroll-reveal', 'reveal-pending');
      observer?.observe(section);
    });
    motion.addEventListener('change', revealAll);
  });
  onUnmounted(() => {
    observer?.disconnect();
    motion?.removeEventListener('change', revealAll);
    sections.forEach((section) => section.classList.remove('scroll-reveal', 'reveal-pending'));
  });
};
