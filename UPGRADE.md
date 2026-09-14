# Upgrade Guide

> Source: `UPGRADE.md` (repo root).  
> Copied to `data/www/html/UPGRADE.md` by `build.sh`,  
> rendered to HTML by `GET /api/docs/upgrade`.

## Before Upgrading

1. **Back up the database**: `cp data/zap.db data/zap.db.bak.$(date +%s)`
2. **Back up the config**: `cp data/zapd.yaml data/zapd.yaml.bak`
3. **Back up sites**: if you use a custom path such as `/var/www`, `tar` it too.

## Upgrade

```bash
curl -fsSL https://get.zap.sh | bash
# or
zapupgrade --to v0.6.0
```

## After Upgrading

1. Open the panel and confirm **Dashboard** and **Changelog** agree.
2. Verify all cron tasks are still present.
3. Diff your custom snippets against the new `nginx.conf` template.

## Rollback

```bash
zapupgrade --rollback
```