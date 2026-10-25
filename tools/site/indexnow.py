"""Tell search engines that the site changed, through IndexNow (Bing, Yandex, Naver, Seznam and others share it).

Usage: python tools/site/indexnow.py            after a deploy, once the key file is live
The key is the name of the <key>.txt file at the site root (site/<key>.txt holds the same key); the URLs come from
site/sitemap.xml. Python standard library only; nothing else is sent.
"""
from pathlib import Path
import json
import re
import sys
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
SITE = ROOT / 'site'
HOST = 'uruskan.github.io'
BASE = f'https://{HOST}/EliteNexus/'

keys = [p for p in SITE.glob('*.txt') if re.fullmatch(r'[0-9a-f]{32}', p.stem) and p.read_text().strip() == p.stem]
if len(keys) != 1:
    sys.exit('Expected exactly one IndexNow key file (site/<32 hex chars>.txt holding its own name).')
key = keys[0].stem
urls = re.findall(r'<loc>(.*?)</loc>', (SITE / 'sitemap.xml').read_text(encoding='utf-8'))
payload = {'host': HOST, 'key': key, 'keyLocation': BASE + key + '.txt', 'urlList': urls}
request = urllib.request.Request('https://api.indexnow.org/IndexNow', data=json.dumps(payload).encode(), headers={'Content-Type': 'application/json; charset=utf-8'})
with urllib.request.urlopen(request, timeout=30) as response:
    print(f'IndexNow: HTTP {response.status} for {len(urls)} URLs')
