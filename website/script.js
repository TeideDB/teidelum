document.addEventListener('DOMContentLoaded', () => {
  'use strict';

  // ── Scroll-triggered fade-in ──────────────────
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

  // ── Mobile nav toggle ─────────────────────────
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

  // ── Nav shadow + active link ──────────────────
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

  // ── Copy button (for static code blocks) ──────
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

  // ── Scroll-to-top ─────────────────────────────
  const scrollTopBtn = document.querySelector('.scroll-top');
  if (scrollTopBtn) {
    window.addEventListener('scroll', () => {
      scrollTopBtn.classList.toggle('visible', window.scrollY > 600);
    }, { passive: true });
    scrollTopBtn.addEventListener('click', () => {
      window.scrollTo({ top: 0, behavior: 'smooth' });
    });
  }

  // ── Hero typing card ───────────────────────────
  const HERO_SCENES = [
    {
      cmd: 'search "authentication redesign"',
      result: [
        '  Authentication Redesign RFC <span class="t-num">0.92</span>  <span class="t-dim">notion</span>',
        '  Rate Limiting Design        <span class="t-num">0.71</span>  <span class="t-dim">notion</span>',
        '  Deployment Runbook v3       <span class="t-num">0.58</span>  <span class="t-dim">notion</span>',
      ]
    },
    {
      cmd: 'sql "SELECT status, count(*) FROM project_tasks GROUP BY status"',
      result: [
        '  <span class="t-dim">status          count</span>',
        '  backlog             <span class="t-num">5</span>',
        '  in_progress         <span class="t-num">8</span>',
        '  done                <span class="t-num">7</span>',
      ]
    },
    {
      cmd: 'graph neighbors "Alice Chen"',
      result: [
        '  <span class="t-dim">team_members:Alice Chen</span>',
        '  \u251c\u2500 assigned_to  <span class="t-num">4 tasks</span>',
        '  \u2514\u2500 reported_by  <span class="t-num">2 incidents</span>',
      ]
    },
  ];

  class HeroTyper {
    constructor(el, scenes) {
      this.el = el;
      this.scenes = scenes;
      this.running = false;
    }

    async start() {
      if (this.running) return;
      this.running = true;
      while (this.running) {
        for (const scene of this.scenes) {
          if (!this.running) return;
          this.el.innerHTML = '';

          // Type prompt + command
          const promptLine = document.createElement('div');
          promptLine.className = 'hero-typer-line';
          this.el.appendChild(promptLine);

          const prompt = '<span class="t-dim">\u203a</span> ';
          const cursor = document.createElement('span');
          cursor.className = 'term-cursor';
          cursor.textContent = '\u2588';

          for (let i = 0; i < scene.cmd.length; i++) {
            if (!this.running) return;
            promptLine.innerHTML = prompt + this.esc(scene.cmd.slice(0, i + 1));
            promptLine.appendChild(cursor);
            await this.wait(28 + Math.random() * 32);
          }
          cursor.remove();
          await this.wait(350);

          // Blank line
          const blank = document.createElement('div');
          blank.className = 'hero-typer-line';
          blank.innerHTML = '\u00a0';
          this.el.appendChild(blank);

          // Result lines
          for (const html of scene.result) {
            if (!this.running) return;
            const line = document.createElement('div');
            line.className = 'hero-typer-line hero-typer-result';
            line.innerHTML = html;
            this.el.appendChild(line);
            await this.wait(70);
          }

          // Hold
          await this.wait(3000);

          // Fade out
          this.el.style.opacity = '0';
          await this.wait(400);
          this.el.style.opacity = '1';
        }
      }
    }

    stop() { this.running = false; }
    esc(t) { return t.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;'); }
    wait(ms) { return new Promise((r) => setTimeout(r, ms)); }
  }

  const heroTyperEl = document.getElementById('hero-typer');
  if (heroTyperEl) {
    new HeroTyper(heroTyperEl, HERO_SCENES).start();
  }

  // ── Terminal typing animation ─────────────────
  const TERMINAL_FRAMES = [
    { type: 'cmd', text: '$ teidelum --port 8080' },
    { type: 'out', lines: [
      '<span class="t-ok">\u2713</span> HTTP server on <span class="t-url">http://127.0.0.1:8080</span>',
      '<span class="t-ok">\u2713</span> MCP stdio ready \u00b7 11 tools registered',
    ]},
    { type: 'wait', ms: 1200 },
    { type: 'cmd', text: '$ curl -s localhost:8080/api/v1/search \\' },
    { type: 'cmd', text: '    -d \'{"query":"authentication redesign","limit":3}\'' },
    { type: 'wait', ms: 500 },
    { type: 'out', lines: [
      '{',
      '  <span class="t-key">"results"</span>: [',
      '    { <span class="t-key">"title"</span>: <span class="t-str">"Authentication Redesign RFC"</span>, <span class="t-key">"score"</span>: <span class="t-num">0.92</span> },',
      '    { <span class="t-key">"title"</span>: <span class="t-str">"Rate Limiting Design"</span>, <span class="t-key">"score"</span>: <span class="t-num">0.71</span> },',
      '    { <span class="t-key">"title"</span>: <span class="t-str">"Deployment Runbook v3"</span>, <span class="t-key">"score"</span>: <span class="t-num">0.58</span> }',
      '  ]',
      '}',
    ]},
    { type: 'wait', ms: 1500 },
    { type: 'cmd', text: '$ curl -s localhost:8080/api/v1/sql \\' },
    { type: 'cmd', text: '    -d \'{"query":"SELECT name, role FROM team_members LIMIT 4"}\'' },
    { type: 'wait', ms: 500 },
    { type: 'out', lines: [
      '{',
      '  <span class="t-key">"columns"</span>: [<span class="t-str">"name"</span>, <span class="t-str">"role"</span>],',
      '  <span class="t-key">"rows"</span>: [',
      '    [<span class="t-str">"Alice Chen"</span>, <span class="t-str">"Engineering Lead"</span>],',
      '    [<span class="t-str">"Bob Martinez"</span>, <span class="t-str">"Senior Backend"</span>],',
      '    [<span class="t-str">"Carol Wu"</span>, <span class="t-str">"Frontend Developer"</span>],',
      '    [<span class="t-str">"Dave Johnson"</span>, <span class="t-str">"DevOps Engineer"</span>]',
      '  ]',
      '}',
    ]},
    { type: 'wait', ms: 4000 },
    { type: 'clear' },
  ];

  class TerminalAnimation {
    constructor(el, frames) {
      this.el = el;
      this.frames = frames;
      this.running = false;
    }

    async start() {
      if (this.running) return;
      this.running = true;
      while (this.running) {
        this.el.innerHTML = '';
        for (const frame of this.frames) {
          if (!this.running) return;
          switch (frame.type) {
            case 'cmd': await this.typeCommand(frame.text); break;
            case 'out': await this.showOutput(frame.lines); break;
            case 'wait': await this.wait(frame.ms); break;
            case 'clear':
              await this.wait(300);
              this.el.style.opacity = '0';
              await this.wait(300);
              this.el.innerHTML = '';
              this.el.style.opacity = '1';
              break;
          }
        }
      }
    }

    stop() {
      this.running = false;
    }

    async typeCommand(text) {
      const line = document.createElement('div');
      line.className = 'term-line term-cmd';
      this.el.appendChild(line);

      const cursor = document.createElement('span');
      cursor.className = 'term-cursor';
      cursor.textContent = '\u2588';

      for (let i = 0; i < text.length; i++) {
        if (!this.running) return;
        line.textContent = text.slice(0, i + 1);
        line.appendChild(cursor);
        this.scrollToBottom();
        await this.wait(25 + Math.random() * 35);
      }

      cursor.remove();
      await this.wait(120);
    }

    async showOutput(lines) {
      for (const html of lines) {
        if (!this.running) return;
        const line = document.createElement('div');
        line.className = 'term-line term-out';
        line.innerHTML = html;
        this.el.appendChild(line);
        this.scrollToBottom();
        await this.wait(35);
      }
    }

    scrollToBottom() {
      this.el.scrollTop = this.el.scrollHeight;
    }

    wait(ms) {
      return new Promise((resolve) => setTimeout(resolve, ms));
    }
  }

  // Start terminal when scrolled into view
  const terminalOutput = document.getElementById('terminal-output');
  if (terminalOutput) {
    const terminal = new TerminalAnimation(terminalOutput, TERMINAL_FRAMES);
    const termObserver = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          terminal.start();
          termObserver.unobserve(entry.target);
        }
      });
    }, { threshold: 0.3 });
    termObserver.observe(terminalOutput);
  }
});
