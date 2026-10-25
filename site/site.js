'use strict';

// The only script on the site: the small-screen menu and its dropdowns. Everything else, screenshots
// included, is plain HTML that works without JavaScript.
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
