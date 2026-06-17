# Deploying the website (Cloudflare Pages → highlightscout.app)

The site is a single static page in `docs/`. It's hosted on **Cloudflare Pages**
and served at **highlightscout.app** (domain managed in Cloudflare).

## One-time setup (dashboard)

1. Cloudflare dashboard → **Workers & Pages → Create → Pages → Connect to Git**.
2. Pick the `highlight-scout` repo.
3. Build settings:
   - **Framework preset:** None
   - **Build command:** *(leave empty — it's static)*
   - **Build output directory:** `docs`
4. Deploy. Every push to `main` now publishes automatically.
5. **Custom domain:** the Pages project → **Custom domains → Set up a custom
   domain → `highlightscout.app`**. Since the domain is already in this
   Cloudflare account, the DNS record is created for you.

## Or deploy from the CLI

```bash
# one-time: create the project (or do it in the dashboard)
npx wrangler pages project create highlight-scout --production-branch main

# deploy the static folder
npx wrangler pages deploy docs --project-name highlight-scout
```

Then attach `highlightscout.app` as a custom domain in the dashboard.

> The app's GitHub **Releases** (built by `.github/workflows/release.yml`) are
> separate from this website — the download buttons just link to them.
