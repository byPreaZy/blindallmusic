/* Blind'All Music service worker.
 *
 * Strategy:
 *  - Hashed static assets (blindall-web-*.js/_bg.wasm, app-*.css, icons, js/*)
 *    -> cache-first (content-addressed, immutable).
 *  - SPA navigations -> network-first, fall back to cached index.html offline.
 *  - /api/* and websockets -> network only.
 *  - External audio previews -> stale-while-revalidate into a bounded LRU cache
 *    so playback survives flaky metro connections.
 *
 * Every respondWith resolves a Response — a rejected promise surfaces to the
 * page as an opaque "network error" even when the real network would have
 * worked (e.g. service-worker transient failure). Response.error() keeps the
 * semantics of a failed fetch without the rejection.
 */

const VERSION = 'v2';
const SHELL_CACHE = `blindall-shell-${VERSION}`;
const RUNTIME_CACHE = `blindall-rt-${VERSION}`;
const AUDIO_CACHE = `blindall-audio-${VERSION}`;
const AUDIO_CACHE_MAX = 80;

const STATIC_RE = /^\/(blindall-web-[^/]+\.(js|wasm)|app-[^/]+\.css|favicon[^/]*\.(png|svg)|apple-touch-icon\.png|manifest\.json|js\/.+|images\/.+)$/;

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches
      .open(SHELL_CACHE)
      .then((c) => c.addAll(['/', '/manifest.json']).catch(() => undefined))
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(
          keys
            .filter((k) => ![SHELL_CACHE, RUNTIME_CACHE, AUDIO_CACHE].includes(k))
            .map((k) => caches.delete(k)),
        ),
      )
      .then(() => self.clients.claim()),
  );
});

async function trimAudioCache() {
  const cache = await caches.open(AUDIO_CACHE);
  const keys = await cache.keys();
  if (keys.length <= AUDIO_CACHE_MAX) return;
  const excess = keys.length - AUDIO_CACHE_MAX;
  for (let i = 0; i < excess; i += 1) {
    await cache.delete(keys[i]);
  }
}

async function staleWhileRevalidate(request) {
  const cache = await caches.open(AUDIO_CACHE).catch(() => null);
  const cached = cache ? await cache.match(request).catch(() => null) : null;
  const fetchPromise = fetch(request)
    .then((response) => {
      if (cache && (response.ok || response.type === 'opaque')) {
        cache.put(request, response.clone()).then(trimAudioCache).catch(() => {});
      }
      return response;
    })
    .catch(() => cached || Response.error());
  return cached || fetchPromise;
}

async function cacheFirst(request) {
  const cached = await caches.match(request).catch(() => null);
  if (cached) return cached;
  try {
    const response = await fetch(request);
    if (response.ok) {
      const cache = await caches.open(RUNTIME_CACHE).catch(() => null);
      if (cache) await cache.put(request, response.clone()).catch(() => {});
    }
    return response;
  } catch {
    return Response.error();
  }
}

async function networkFirstShell(request) {
  const cache = await caches.open(SHELL_CACHE).catch(() => null);
  try {
    const response = await fetch(request);
    if (cache && response.ok) {
      await cache.put('/', response.clone()).catch(() => {});
      await cache.put(request, response.clone()).catch(() => {});
    }
    return response;
  } catch {
    if (cache) {
      const cached = await cache.match(request).catch(() => null);
      if (cached) return cached;
      const shell = await cache.match('/').catch(() => null);
      if (shell) return shell;
    }
    return new Response('Hors connexion', {
      status: 503,
      headers: { 'Content-Type': 'text/plain; charset=utf-8' },
    });
  }
}

self.addEventListener('fetch', (event) => {
  const { request } = event;
  if (request.method !== 'GET') return;

  const url = new URL(request.url);

  // Same-origin handling
  if (url.origin === self.location.origin) {
    // Never cache API responses or realtime traffic.
    if (url.pathname.startsWith('/api/') || url.pathname.startsWith('/ws')) {
      return;
    }
    if (request.mode === 'navigate' || url.pathname === '/') {
      event.respondWith(networkFirstShell(request));
      return;
    }
    if (STATIC_RE.test(url.pathname)) {
      event.respondWith(cacheFirst(request));
      return;
    }
    // Locally hosted tracks/artwork: same SWR audio cache so uploaded excerpts
    // keep playing on flaky metro connections.
    if (
      url.pathname.startsWith('/storage/') &&
      /\.(mp3|m4a|aac|ogg|wav|png|jpe?g|webp|svg)(\?|$)/i.test(url.pathname)
    ) {
      event.respondWith(staleWhileRevalidate(request));
      return;
    }
    // Other same-origin GETs (e.g. images referenced later): SWR-light.
    // Resolve Response.error() rather than rejecting the respondWith promise.
    event.respondWith(
      caches
        .match(request)
        .catch(() => null)
        .then((cached) => cached || fetch(request))
        .catch(() => Response.error()),
    );
    return;
  }

  // External audio previews: keep them available on flaky networks.
  if (/\.(mp3|m4a|aac|ogg|wav)(\?|$)/.test(url.pathname) || url.hostname.includes('cdns-preview')) {
    event.respondWith(staleWhileRevalidate(request));
  }
});
