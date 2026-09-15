# Upgrade Guide

> Source: `UPGRADE.md` (repo root).  
> Copied to `data/www/html/UPGRADE.md` by `build.sh`,  
> rendered to HTML by `GET /api/docs/upgrade`.

## Before Upgrading

1. **Back up the data**: `cp -Rf data/  /root/data.bak.$(date +%s)`
2. **Back up the config**: `cp -Rf /etc/zap/ /root/zap.bak.$(date +%s)`

## Upgrade

```bash
curl -fsSL https://get.zap.sh | bash
# or
Login to the zapd server and click the "Upgrade" button in the panel. 
```

## After Upgrading

1. Open the panel and confirm **Dashboard** and **Changelog** agree.
2. Verify all cron tasks are still present.

