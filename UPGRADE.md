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

## Settings Moved Out of the Database

Two settings are now stored as YAML files next to `zap.db` instead of SQL tables:

| File | Contents |
| --- | --- | --- |
| `data/server_env.yaml`   | environment snapshot (`auto`) + panel defaults (`conf`) |
| `data/update_config.yaml` | auto-update switch / cron / channel / last check result |

Both files are loaded at start-up (created with defaults when missing) and written back
atomically immediately after every change; external edits are picked up by mtime without
restarting the panel.

Any leftover `server_env` / `update_config` tables are dropped on the first start after
the upgrade and their rows are **not** migrated — the environment is re-detected and the
auto-update settings can be re-saved from **System Settings → System Update**.

