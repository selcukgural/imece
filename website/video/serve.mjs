// One origin for the tour: the terminal (ttyd, upstream) and the title
// cards (this folder), so PromoVideo's `goto` can reach both. Everything that
// is not a card or the brand mark is passed on to ttyd, websocket included.
//
//   node website/video/serve.mjs [port=8791] [ttydPort=8790]
//
// No dependencies: Node's http module proxies the request, and the upgrade
// for ttyd's websocket is forwarded by hand.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const port = Number(process.argv[2] ?? 8791);
const upstream = Number(process.argv[3] ?? 8790);

// A blank page on the tour's ground: PromoVideo opens /<language> before it
// starts recording, so the first frame is not a 404.
const blank =
  '<!doctype html><meta charset="utf-8"><title>imece</title><body style="margin:0;background:#15161b"></body>\n';

const files = new Map([
  ['/card.html', ['card.html', 'text/html; charset=utf-8']],
  ['/card.css', ['card.css', 'text/css; charset=utf-8']],
  ['/card.js', ['card.js', 'text/javascript; charset=utf-8']],
  // PromoVideo's own `card` step asks for this name; the mark is an SVG,
  // which the browser reads by its content type, not its extension.
  ['/brand/logo-mark.png', ['../static/img/imece-mark.svg', 'image/svg+xml']],
  ['/brand/imece-mark.svg', ['../static/img/imece-mark.svg', 'image/svg+xml']],
]);

const server = http.createServer((req, res) => {
  const url = new URL(req.url, `http://127.0.0.1:${port}`);
  if (url.pathname === '/en' || url.pathname === '/tr' || url.pathname === '/en/' || url.pathname === '/tr/') {
    res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    res.end(blank);
    return;
  }
  const file = files.get(url.pathname);
  if (file) {
    const [name, type] = file;
    fs.readFile(path.join(here, name), (err, body) => {
      if (err) {
        res.writeHead(404);
        res.end();
        return;
      }
      res.writeHead(200, { 'content-type': type, 'cache-control': 'no-store' });
      res.end(body);
    });
    return;
  }
  // The terminal page gets a 6 px #corner inside the terminal's own element,
  // at its bottom right: the tour clicks there to type, so the pointer and
  // its ripple stay out of the way, and the click still focuses the terminal.
  const isPage = url.pathname === '/' || url.pathname === '/index.html';
  const headers = { ...req.headers };
  if (isPage) delete headers['accept-encoding'];
  const out = http.request(
    { host: '127.0.0.1', port: upstream, method: req.method, path: req.url, headers },
    (up) => {
      if (!isPage || !/text\/html/.test(up.headers['content-type'] ?? '')) {
        res.writeHead(up.statusCode ?? 502, up.headers);
        up.pipe(res);
        return;
      }
      const chunks = [];
      up.on('data', (c) => chunks.push(c));
      up.on('end', () => {
        let html = Buffer.concat(chunks).toString('utf8');
        html = html.replace(
          '</body>',
          // The page must not scroll: xterm.js keeps a hidden textarea at the
          // cursor, and the browser scrolls it into view when the cursor is on
          // the last row, cutting the header off the top of the frame.
          // overflow:hidden alone does not stop a scrollIntoView, so every
          // scroll, on the window or on any element, is put back to 0. ttyd
          // fits the terminal before its client options (the font, the line
          // height) arrive, so a resize is raised after them to fit again.
          `<style>html,body{margin:0;height:100%;overflow:hidden}</style><script>(function(){document.addEventListener('scroll',function(e){var el=e.target===document?document.scrollingElement:e.target;if(el&&el.scrollTop!==undefined){el.scrollTop=0;el.scrollLeft=0;}},true);var t=setInterval(function(){var x=document.querySelector('.xterm');if(!x)return;clearInterval(t);var s=document.createElement('span');s.id='corner';s.style.cssText='position:fixed;right:3px;bottom:3px;width:6px;height:6px;z-index:10';x.appendChild(s);[800,2000,4000].forEach(function(ms){setTimeout(function(){window.dispatchEvent(new Event('resize'));},ms);});},100);})();</script></body>`,
        );
        const h = { ...up.headers };
        delete h['content-length'];
        delete h['content-encoding'];
        res.writeHead(up.statusCode ?? 200, h);
        res.end(html);
      });
    },
  );
  out.on('error', () => {
    res.writeHead(502);
    res.end('ttyd is not up');
  });
  req.pipe(out);
});

server.on('upgrade', (req, socket, head) => {
  const out = http.request({
    host: '127.0.0.1',
    port: upstream,
    method: req.method,
    path: req.url,
    headers: req.headers,
  });
  out.on('upgrade', (up, upSocket, upHead) => {
    const lines = [`HTTP/1.1 ${up.statusCode} ${up.statusMessage}`];
    for (const [k, v] of Object.entries(up.headers)) {
      for (const one of Array.isArray(v) ? v : [v]) lines.push(`${k}: ${one}`);
    }
    socket.write(lines.join('\r\n') + '\r\n\r\n');
    if (upHead.length) socket.write(upHead);
    if (head.length) upSocket.write(head);
    upSocket.pipe(socket);
    socket.pipe(upSocket);
    const close = () => {
      socket.destroy();
      upSocket.destroy();
    };
    socket.on('error', close);
    upSocket.on('error', close);
    socket.on('close', close);
    upSocket.on('close', close);
  });
  out.on('error', () => socket.destroy());
  out.end();
});

server.listen(port, '127.0.0.1', () => {
  console.log(`Serving the tour on http://127.0.0.1:${port}/ (terminal from :${upstream})`);
});
