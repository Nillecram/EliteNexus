'use strict';

// Full-size image links and page content work without JavaScript, too.
const menuButton = document.querySelector('.menu-toggle');
const navigation = document.querySelector('.primary-nav');
function closeMenu() {
  navigation.classList.remove('is-open');
  menuButton.setAttribute('aria-expanded', 'false');
  menuButton.setAttribute('aria-label', 'Open navigation');
}
menuButton.addEventListener('click', () => {
  const open = navigation.classList.toggle('is-open');
  menuButton.setAttribute('aria-expanded', String(open));
  menuButton.setAttribute('aria-label', open ? 'Close navigation' : 'Open navigation');
});
navigation.addEventListener('click', event => {
  if (event.target.closest('a')) closeMenu();
});
const dropdowns = [...document.querySelectorAll('.nav-dropdown')];
dropdowns.forEach(dropdown => dropdown.addEventListener('toggle', () => {
  if (dropdown.open) dropdowns.forEach(other => { if (other !== dropdown) other.open = false; });
}));
document.addEventListener('click', event => {
  if (!event.target.closest('.nav-dropdown')) dropdowns.forEach(dropdown => { dropdown.open = false; });
  if (!event.target.closest('.site-header')) closeMenu();
});
document.addEventListener('keydown', event => {
  if (event.key !== 'Escape') return;
  const openDropdown = dropdowns.find(dropdown => dropdown.open);
  if (openDropdown) {
    openDropdown.open = false;
    openDropdown.querySelector('summary').focus();
  } else if (navigation.classList.contains('is-open')) {
    closeMenu();
    menuButton.focus();
  }
});
window.matchMedia('(min-width: 1001px)').addEventListener('change', closeMenu);

// Progressively enhance product screenshots into an accessible tabbed tour.
const tour = document.querySelector('[data-tour]');
if (tour) {
  const tabs = [...tour.querySelectorAll('[role="tab"]')];
  const selectTab = (tab, moveFocus = false) => {
    tabs.forEach(item => {
      const selected = item === tab;
      item.setAttribute('aria-selected', String(selected));
      item.tabIndex = selected ? 0 : -1;
      document.getElementById(item.getAttribute('aria-controls')).hidden = !selected;
    });
    if (moveFocus) tab.focus();
  };
  tabs.forEach((tab, index) => {
    tab.addEventListener('click', () => selectTab(tab));
    tab.addEventListener('keydown', event => {
      let next;
      if (event.key === 'ArrowRight') next = (index + 1) % tabs.length;
      else if (event.key === 'ArrowLeft') next = (index - 1 + tabs.length) % tabs.length;
      else if (event.key === 'Home') next = 0;
      else if (event.key === 'End') next = tabs.length - 1;
      else return;
      event.preventDefault();
      selectTab(tabs[next], true);
    });
  });
  selectTab(tabs[0]);
}

// Native dialog supplies modal focus containment, Escape and focus restoration.
const dialog = document.querySelector('.lightbox');
const imageLinks = [...document.querySelectorAll('a[data-lightbox]')];
if (dialog && typeof dialog.showModal === 'function') {
  const slides = [...new Map(imageLinks.map(link => [link.href, link])).values()];
  const image = dialog.querySelector('.lightbox-stage img');
  const caption = dialog.querySelector('#lightbox-caption');
  const original = dialog.querySelector('[data-lightbox-original]');
  const zoom = dialog.querySelector('[data-lightbox-zoom]');
  const status = dialog.querySelector('[data-lightbox-status]');
  const stage = dialog.querySelector('.lightbox-stage');
  let active = 0;
  let trigger;
  const setZoom = value => {
    dialog.classList.toggle('is-zoomed', value);
    zoom.setAttribute('aria-pressed', String(value));
    zoom.textContent = value ? 'Fit to screen' : 'Zoom in';
    stage.tabIndex = value ? 0 : -1;
    stage.scrollTo(0, 0);
    if (value) stage.focus({preventScroll: true});
  };
  const showSlide = index => {
    active = (index + slides.length) % slides.length;
    const link = slides[active];
    image.src = link.href;
    image.alt = link.querySelector('img').alt;
    caption.textContent = link.dataset.caption || image.alt;
    original.href = link.href;
    status.textContent = `Screenshot ${active + 1} of ${slides.length}`;
    setZoom(false);
  };
  imageLinks.forEach(link => link.addEventListener('click', event => {
    if (event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
    event.preventDefault();
    trigger = link;
    showSlide(slides.findIndex(slide => slide.href === link.href));
    dialog.showModal();
    document.body.classList.add('lightbox-open');
    dialog.querySelector('[data-lightbox-close]').focus();
  }));
  dialog.querySelector('[data-lightbox-close]').addEventListener('click', () => dialog.close());
  dialog.querySelector('[data-lightbox-prev]').addEventListener('click', () => showSlide(active - 1));
  dialog.querySelector('[data-lightbox-next]').addEventListener('click', () => showSlide(active + 1));
  zoom.addEventListener('click', () => setZoom(!dialog.classList.contains('is-zoomed')));
  dialog.addEventListener('click', event => { if (event.target === stage) dialog.close(); });
  dialog.addEventListener('keydown', event => {
    if (dialog.classList.contains('is-zoomed')) return;
    if (event.key === 'ArrowRight') { event.preventDefault(); showSlide(active + 1); }
    if (event.key === 'ArrowLeft') { event.preventDefault(); showSlide(active - 1); }
  });
  dialog.addEventListener('close', () => {
    document.body.classList.remove('lightbox-open');
    setZoom(false);
    if (trigger && !trigger.closest('[hidden]')) trigger.focus({preventScroll: true});
  });
  if (slides.length < 2) {
    dialog.querySelector('[data-lightbox-prev]').hidden = true;
    dialog.querySelector('[data-lightbox-next]').hidden = true;
  }
}
