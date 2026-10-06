---
name: release-check
description: Checks a build before release (links, Lighthouse, sitemap).
---
1. `pnpm build`
2. Check for broken links in `build/`
3. Report results with `wly task note`
