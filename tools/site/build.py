"""Build Elite Nexus's static GitHub Pages site. Python standard library only."""
from pathlib import Path
import argparse
import html
import json
import re
import shutil

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'site'
CONTENT = Path(__file__).parent / 'content'
BASE = 'https://uruskan.github.io/EliteNexus/'
REPO = 'https://github.com/uruskan/EliteNexus'
DOWNLOAD = REPO + '/releases/latest'
SPONSOR = 'https://github.com/sponsors/uruskan'

def escape(value):
    return html.escape(str(value), quote=True)

def shot(filename, alt, caption, eager=False):
    return f'''<figure class="screenshot"><a href="screenshots/{escape(filename)}" data-lightbox data-caption="{escape(caption)}" aria-label="Enlarge screenshot: {escape(alt)}"><img src="screenshots/{escape(filename)}" width="1480" height="920" alt="{escape(alt)}" loading="{'eager' if eager else 'lazy'}" decoding="async"></a><figcaption>{escape(caption)}<span class="enlarge-hint" aria-hidden="true">View full size ↗</span></figcaption></figure>'''

def header(slug):
    def link(href, label):
        active = ' aria-current="page"' if href == slug + '.html' else ''
        return f'<a href="{href}"{active}>{label}</a>'
    return f'''<a class="skip-link" href="#main">Skip to content</a>
<header class="site-header"><div class="header-inner">
<a class="brand" href="index.html" aria-label="Elite Nexus home"><span class="brand-mark" aria-hidden="true"></span><span>Elite Nexus</span></a>
<button class="menu-toggle" aria-expanded="false" aria-controls="primary-nav" aria-label="Open navigation"><span></span><span></span><span></span></button>
<nav id="primary-nav" class="primary-nav" aria-label="Main navigation">
{link('index.html#features', 'Features')}
<details class="nav-dropdown"><summary>Cockpit <span aria-hidden="true">⌄</span></summary><div class="dropdown-panel">{link('hud-colours.html', 'HUD colours')}{link('cockpit-lights.html', 'Cockpit lights')}{link('odyssey-glare.html', 'Glare &amp; exposure')}</div></details>
{link('trading.html', 'Trading')}
<details class="nav-dropdown"><summary>Explore <span aria-hidden="true">⌄</span></summary><div class="dropdown-panel">{link('exploration.html', 'Exploration &amp; navigation')}{link('exobiology.html', 'Exobiology')}</div></details>
<details class="nav-dropdown"><summary>Commander <span aria-hidden="true">⌄</span></summary><div class="dropdown-panel">{link('commander.html', 'Fleet &amp; engineering')}{link('companion.html', 'AI &amp; everyday tools')}{link('alternatives.html', 'Why Elite Nexus')}</div></details>
<a class="nav-source" href="{REPO}">GitHub <span aria-hidden="true">↗</span></a>
<a class="button button-small" href="{'#download' if slug == 'index' else 'index.html#download'}">Download <span aria-hidden="true">↓</span></a>
</nav></div></header>'''

def footer():
    return f'''<footer class="site-footer"><div class="container footer-grid">
<div class="footer-brand"><a class="brand" href="index.html"><span class="brand-mark" aria-hidden="true"></span><span>Elite Nexus</span></a><p>One app. Your whole galaxy.</p><span class="small-label">Free software · Native app for Windows and Linux</span></div>
<div><h2>Make it yours</h2><a href="hud-colours.html">HUD colours</a><a href="cockpit-lights.html">Cockpit lights</a><a href="odyssey-glare.html">Glare &amp; exposure</a></div>
<div><h2>Find your next move</h2><a href="trading.html">Trading &amp; markets</a><a href="exploration.html">Exploration &amp; navigation</a><a href="exobiology.html">Exobiology</a><a href="commander.html">Fleet &amp; engineering</a><a href="companion.html">AI &amp; everyday tools</a></div>
<div><h2>Get involved</h2><a href="{DOWNLOAD}">Download &amp; release notes</a><a href="{REPO}">Source on GitHub</a><a href="{SPONSOR}">Sponsor on GitHub</a><a href="{REPO}/blob/main/docs/USER_GUIDE.md">User guide</a><a href="{REPO}/issues">Report an issue</a><a href="{REPO}/blob/main/LICENSE">GPL-3.0 licence</a></div>
</div><div class="container footer-bottom"><p>An independent community companion. Not affiliated with or endorsed by Frontier Developments.<br>Elite Dangerous is a trademark of Frontier Developments plc.</p><a href="#top">Back to top ↑</a></div></footer>
<dialog class="lightbox" aria-labelledby="lightbox-caption"><div class="lightbox-layout"><div class="lightbox-toolbar"><p id="lightbox-caption"></p><div class="lightbox-actions"><button type="button" data-lightbox-prev aria-label="Previous screenshot">←</button><button type="button" data-lightbox-next aria-label="Next screenshot">→</button><button type="button" data-lightbox-zoom aria-pressed="false">Zoom in</button><a data-lightbox-original target="_blank" rel="noopener">Original ↗</a><button type="button" data-lightbox-close aria-label="Close screenshot">✕</button></div></div><div class="lightbox-stage"><img alt=""></div><p class="lightbox-help">Arrow keys to browse · Escape to close</p><p class="sr-only" aria-live="polite" data-lightbox-status></p></div></dialog>'''

def page_body(page):
    if page['slug'] == 'index':
        return (CONTENT / 'index.html').read_text(encoding='utf-8')
    body = (CONTENT / (page['slug'] + '.html')).read_text(encoding='utf-8')
    anchors = re.findall(r'<section\b[^>]*\bid=[\"\x27]([^\"\x27]+)[\"\x27][^>]*>(.*?)(?=<section\b|$)', body, re.S)
    jump_links = []
    for anchor, section in anchors:
        heading = re.search(r'<h2\b[^>]*>(.*?)</h2>', section, re.S)
        if heading:
            label = re.sub('<[^>]+>', '', heading.group(1))
            jump_links.append(f'<a href="#{escape(anchor)}">{label}</a>')
    if page.get('toc'):
        jump_links = [f'<a href="#{escape(item["id"])}">{escape(item["label"])}</a>' for item in page['toc']]
    jumps = '<nav class="page-jump container" aria-label="On this page"><span>ON THIS PAGE</span>' + ''.join(jump_links) + '</nav>' if jump_links else ''
    hero = f'''<section class="page-hero container"><div><a class="back-link" href="index.html#features">← All features</a><p class="eyebrow">{escape(page['kicker'])}</p><h1>{escape(page['heading'])}</h1><p class="lead">{escape(page['lead'])}</p><a class="button" href="{DOWNLOAD}">Download for Windows &amp; Linux <span aria-hidden="true">↓</span></a></div><div class="page-hero-image">{shot(page['hero_image'], page['hero_alt'], page['hero_caption'], True)}</div></section>'''
    related = ''.join(f'<a class="related-link" href="{escape(item["href"])}"><h3>{escape(item["label"])} <span aria-hidden="true">↗</span></h3><p>{escape(item["description"])}</p></a>' for item in page.get('related', []))
    closing = f'''<section class="section container"><div class="section-heading"><p class="eyebrow">Keep exploring</p><h2>More of your galaxy, connected.</h2></div><div class="related-grid">{related}</div></section><section class="container compact-cta"><div><p class="eyebrow">Ready when you are</p><h2>Your next session starts here.</h2><p>Free and open source. Windows 10/11 or Linux, 64-bit. Unpack and fly.</p></div><a class="button" href="{DOWNLOAD}">Get Elite Nexus <span aria-hidden="true">↓</span></a></section>'''
    return hero + jumps + body + closing

def render(page):
    slug = page['slug']
    url = BASE if slug == 'index' else BASE + slug + '.html'
    schema = {'@context': 'https://schema.org', '@type': 'SoftwareApplication', 'name': 'Elite Nexus', 'applicationCategory': 'GameApplication', 'operatingSystem': 'Windows 10, Windows 11, Linux (64-bit)', 'description': page['description'], 'url': BASE, 'downloadUrl': DOWNLOAD, 'codeRepository': REPO, 'license': REPO + '/blob/main/LICENSE', 'offers': {'@type': 'Offer', 'price': '0', 'priceCurrency': 'USD'}}
    return f'''<!doctype html>
<html lang="en" id="top"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta name="theme-color" content="#0b0d10"><meta name="google-site-verification" content="qddrMPdC8qXkPKaDAJsy4fdm2HvshZ6FeSoM-YSG02w">
<title>{escape(page['title'])}</title><meta name="description" content="{escape(page['description'])}"><link rel="canonical" href="{url}">
<meta property="og:type" content="website"><meta property="og:site_name" content="Elite Nexus"><meta property="og:title" content="{escape(page['title'])}"><meta property="og:description" content="{escape(page['description'])}"><meta property="og:url" content="{url}"><meta property="og:image" content="{BASE}assets/social-preview.jpg"><meta property="og:image:width" content="1280"><meta property="og:image:height" content="640"><meta property="og:image:alt" content="Elite Nexus: cockpit customization, trading, exploration, engineering and more in one companion app for Windows and Linux."><meta name="twitter:card" content="summary_large_image">
<link rel="icon" href="assets/favicon.svg" type="image/svg+xml"><link rel="preload" href="fonts/Inter-Regular.ttf" as="font" type="font/ttf" crossorigin><link rel="stylesheet" href="style.css"><script>document.documentElement.classList.add('js');</script><script src="site.js" defer></script>
<script type="application/ld+json">{json.dumps(schema)}</script></head><body class="{'home' if slug == 'index' else 'detail-page'}">
{header(slug)}<main id="main">{page_body(page)}</main>{footer()}
</body></html>\n'''

def build(destination):
    protected = (SOURCE / 'assets', ROOT / 'docs', ROOT / 'data', ROOT / 'tools', ROOT / '.git')
    if any(destination.is_relative_to(folder.resolve()) for folder in protected):
        raise ValueError('Output must not be inside a source asset, documentation, tools or Git directory.')
    destination.mkdir(parents=True, exist_ok=True)
    pages = [json.loads(p.read_text(encoding='utf-8')) for p in sorted(CONTENT.glob('*.json'))]
    for page in pages:
        if not re.fullmatch(r'[a-z0-9]+(?:-[a-z0-9]+)*', page['slug']):
            raise ValueError(f'Invalid page slug: {page["slug"]!r}')
        (destination / (page['slug'] + '.html')).write_text(render(page), encoding='utf-8')
    if destination.resolve() != SOURCE.resolve():
        for name in ('style.css', 'site.js', 'robots.txt', 'llms.txt'):
            shutil.copy2(SOURCE / name, destination / name)
        shutil.copytree(SOURCE / 'assets', destination / 'assets', dirs_exist_ok=True)
    for source, target in [(ROOT / 'docs/screenshots', destination / 'screenshots'), (ROOT / 'data/fonts/inter', destination / 'fonts')]:
        shutil.copytree(source, target, dirs_exist_ok=True)
    shutil.copy2(ROOT / 'docs/social-preview.png', destination / 'social-preview.png')
    shutil.copy2(ROOT / 'docs/USER_GUIDE.md', destination / 'user-guide.md')
    urls = ''.join(f'<url><loc>{BASE if p["slug"] == "index" else BASE + p["slug"] + ".html"}</loc></url>\n' for p in pages)
    (destination / 'sitemap.xml').write_text('<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n' + urls + '</urlset>\n', encoding='utf-8')
    print(f'Built {len(pages)} pages in {destination}')

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=SOURCE)
    build(parser.parse_args().output.resolve())
