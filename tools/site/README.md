# Website

The GitHub Pages site is static HTML, CSS and JavaScript. It has no runtime framework, external font service,
analytics or package-install step.

## Edit and build

- Edit the homepage body in `tools/site/content/index.html` and its metadata in `index.json`.
- Feature-page copy lives in `tools/site/content/*.html`; the matching JSON files hold metadata and related links.
- `tools/site/build.py` owns the shared head, navigation, page layout, related links and footer.
- Design and interactions live in `site/style.css` and `site/site.js`.
- `site/assets/` contains original website artwork and the diamond favicon.
- App screenshots remain canonical in `docs/screenshots/`; Inter and its licence remain in `data/fonts/inter/`.

Run with Python 3 (standard library only):

```text
python tools/site/build.py
python tools/site/check.py --root site
python -m http.server 4173 --bind 127.0.0.1 --directory site
```

The default build refreshes the tracked HTML and sitemap in `site/` and copies ignored screenshots, fonts and
guide assets for local viewing. Do not hand-edit the generated HTML. To assemble a clean deployment directory,
use `python tools/site/build.py --output PATH`. The GitHub Pages workflow uses `_site` and runs the link/content
checker before publishing. All public URLs work under the `/EliteNexus/` project path.

The checker catches broken pages, assets and anchors, duplicate IDs, missing metadata, incorrect production
canonicals, and screenshot links without full-size viewing or meaningful alt text. Browser checks should also
cover mobile navigation, all product-tour tabs, lightbox zoom/next/previous/Escape and keyboard focus restoration.
Images remain ordinary full-size links and all product-tour panels remain visible when JavaScript is disabled.

## Quote collage

The homepage's "What commanders are saying" section and `docs/testimonials.png` (in the repository README, and for
posting elsewhere) carry the same quotes from the launch threads. Keep them in step: the section lives in
`content/index.html`, the image's source in `testimonials.html`. Quotes stay word for word, with "…" where one is
shortened, and without usernames or avatars: the commanders wrote on a forum, not for an advert.

To redraw the image, run `python tools/site/testimonials.py` and open the URL it prints in a Chromium-based browser
(`--open` uses your default browser). The page draws itself at 2× through the browser's own renderer and posts the
PNG back; the script saves it to `docs/testimonials.png` (2800 × 2200) and exits. Headless `--screenshot` would be
simpler, but it can't start inside an agent's command sandbox.

## Artwork

`galaxy-hero.webp` is original artwork made with the built-in image generation tool for this site. It is conceptual
space imagery; it does not depict a game screenshot. Generation brief: a text-free, very wide charcoal space scene,
planetary horizon at lower right, blue atmospheric rim, restrained amber sunrise and orbital routes, an almost
empty left half for legible headings, no ships, official logos or fabricated interface. Exported to WebP quality 86.

`social-preview.jpg` is the approved feature-rich Elite Nexus artwork from this chat, exported at 1280 × 640.
