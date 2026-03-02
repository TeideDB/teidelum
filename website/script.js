document.addEventListener('DOMContentLoaded', () => {
  'use strict';

  // Scroll-triggered fade-in
  const animatedEls = document.querySelectorAll('.feature-item, .project-card, .fade-in-up');
  if (animatedEls.length > 0) {
    const observer = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          const parent = entry.target.parentElement;
          if (parent) {
            const siblings = parent.querySelectorAll('.feature-item, .project-card, .fade-in-up');
            const index = Array.prototype.indexOf.call(siblings, entry.target);
            if (index > 0) {
              entry.target.style.transitionDelay = (index * 0.12) + 's';
            }
          }
          entry.target.classList.add('visible');
          observer.unobserve(entry.target);
        }
      });
    }, { threshold: 0.15 });
    animatedEls.forEach((el) => observer.observe(el));
  }

  // Mobile nav toggle
  const navToggle = document.querySelector('.nav-toggle');
  const navLinks = document.querySelector('.nav-links');
  if (navToggle && navLinks) {
    navToggle.addEventListener('click', () => {
      const isOpen = navLinks.classList.toggle('open');
      navToggle.classList.toggle('open');
      navToggle.setAttribute('aria-expanded', String(isOpen));
    });
    navLinks.querySelectorAll('a').forEach((link) => {
      link.addEventListener('click', () => {
        navLinks.classList.remove('open');
        navToggle.classList.remove('open');
        navToggle.setAttribute('aria-expanded', 'false');
      });
    });
  }

  // Nav shadow on scroll + active link highlighting
  const nav = document.querySelector('.nav');
  const sections = document.querySelectorAll('section[id]');
  const navAnchors = document.querySelectorAll('.nav-links a[href^="#"]');
  function updateNav() {
    if (nav) {
      nav.classList.toggle('nav-scrolled', window.scrollY > 50);
    }
    let currentId = '';
    const scrollY = window.scrollY + 120;
    sections.forEach((section) => {
      const top = section.offsetTop;
      const height = section.offsetHeight;
      if (scrollY >= top && scrollY < top + height) {
        currentId = section.getAttribute('id');
      }
    });
    navAnchors.forEach((a) => {
      a.classList.toggle('active', a.getAttribute('href') === '#' + currentId);
    });
  }
  window.addEventListener('scroll', updateNav, { passive: true });
  updateNav();

  // Copy button
  document.querySelectorAll('.copy-btn').forEach((btn) => {
    btn.addEventListener('click', () => {
      const codeBlock = btn.closest('.code-block');
      if (!codeBlock) return;
      const code = codeBlock.querySelector('code');
      if (!code) return;
      navigator.clipboard.writeText(code.textContent).then(() => {
        btn.classList.add('copied');
        btn.innerHTML = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg> Copied!';
        setTimeout(() => {
          btn.classList.remove('copied');
          btn.innerHTML = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/></svg> Copy';
        }, 2000);
      });
    });
  });

  // Scroll-to-top
  const scrollTopBtn = document.querySelector('.scroll-top');
  if (scrollTopBtn) {
    window.addEventListener('scroll', () => {
      scrollTopBtn.classList.toggle('visible', window.scrollY > 600);
    }, { passive: true });
    scrollTopBtn.addEventListener('click', () => {
      window.scrollTo({ top: 0, behavior: 'smooth' });
    });
  }
});
