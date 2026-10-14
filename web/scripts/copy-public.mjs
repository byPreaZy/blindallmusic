// Trunk post_build : trunk's `copy-dir` would nest `public/` under
// `dist/public/`; Laravel serves these files at the site root instead
// (/favicon.svg, /manifest.json, /ads.txt, /js/lame.all.js, /images/*).
// CWD isn't guaranteed : resolve through trunk's own env vars.
import { cpSync, existsSync } from 'node:fs'
import { join } from 'node:path'

const src = join(process.env.TRUNK_SOURCE_DIR ?? '.', 'public')
// post_build runs before the staging dir is swapped into dist/, copy into
// the staging dir (what becomes dist), never dist itself.
const dest = process.env.TRUNK_STAGING_DIR ?? join(process.env.TRUNK_DIST_DIR ?? 'dist')

if (existsSync(src)) {
  cpSync(src, dest, { recursive: true })
  console.log(`copy-public: ${src} -> ${dest}`)
} else {
  console.warn(`copy-public: no ${src} (TRUNK_SOURCE_DIR=${process.env.TRUNK_SOURCE_DIR})`)
}
