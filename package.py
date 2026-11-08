#!/usr/bin/env python3
"""Build Glance Dashboard into one installable MIRA HTML file.

Usage (from the mira-sdk folder that contains Shared/ and CompanionExample/):
    rustup target add wasm32-unknown-unknown
    python3 GlanceDashboard/package.py GlanceDashboard.html
"""
from pathlib import Path
import base64, subprocess, sys, tempfile

root = Path(__file__).resolve().parent
sys.path.insert(0, str(root.parent / 'CompanionExample'))
from validate import validate_html  # noqa: E402  (SDK validator)

output = Path(sys.argv[1] if len(sys.argv) > 1 else 'GlanceDashboard.html')
companion = (root / 'companion.html').read_text(encoding='utf-8')
# Optional live calendar + inbox: put your Apps Script web app URL (ending in
# ?key=YOUR_KEY) on one line in GlanceDashboard/feed-url.txt.
feed_file = root / 'feed-url.txt'
feed = feed_file.read_text(encoding='utf-8').strip() if feed_file.exists() else ''
if feed:
    assert feed.startswith('https://script.google.com/') and '?key=' in feed, \
        'feed-url.txt should hold https://script.google.com/macros/s/.../exec?key=YOUR_KEY'
    assert "'" not in feed and '\\' not in feed, 'feed URL contains an unexpected character'
    companion = companion.replace('__GLANCE_FEED_URL__', feed)
    print('Live feed: on')
else:
    print('Live feed: off (no GlanceDashboard/feed-url.txt)')
with tempfile.TemporaryDirectory(prefix='glance-dashboard-') as folder:
    module = Path(folder) / 'dashboard.wasm'
    subprocess.run(['rustc', str(root / 'dashboard.rs'), '--edition=2021', '--target=wasm32-unknown-unknown',
                    '--crate-type=cdylib', '-C', 'opt-level=z', '-C', 'panic=abort', '-C', 'lto=fat',
                    '-C', 'codegen-units=1', '-C', 'link-arg=--no-entry', '-C', 'link-arg=--strip-all',
                    '-C', 'target-feature=-bulk-memory,-bulk-memory-opt,-reference-types',
                    '-C', 'link-arg=-zstack-size=8192', '-C', 'link-arg=--initial-memory=65536',
                    '-C', 'link-arg=--max-memory=65536', '-o', str(module)], check=True)
    encoded = base64.b64encode(module.read_bytes()).decode()
    marker = '</body>'
    assert companion.count(marker) == 1, 'companion.html must contain one </body>'
    html = companion.replace(marker, '<script type="application/mira-wasm">' + encoded + '</script>\n' + marker)
    validate_html(html)
    output.write_text(html, encoding='utf-8')
print(output)
