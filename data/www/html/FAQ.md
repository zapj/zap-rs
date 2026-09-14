# FAQ

> Source: `FAQ.md` (repo root).  
> Copied to `data/www/html/FAQ.md` by `build.sh`,  
> rendered to HTML by `GET /api/docs/faq`.

## Q1: Login fails / token expired

- Make sure cookies are not disabled in the browser.
- Confirm `data/zapd.yaml`'s `jwt_secret` was not reset on restart.

## Q2: New site returns 502

- Confirm PHP-FPM is running.
- Confirm the site `path` exists with the right permissions.

## Q3: Some menu items vanished after upgrade

- Check whether `data/zap.db` was touched by the migration script; during
  development, just rebuild the database from the seeds in `init_db.rs`.

## Q4: API CORS returns 401

- Either turn off `cookie_only` in **System → Zap → JWT**, or set
  `Access-Control-Allow-Credentials` properly on the reverse proxy.