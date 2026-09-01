# 用户自定义包与脚本

此目录存放用户自己创建的包和脚本，**升级官方软件库时永远不会被覆盖**。

## 目录结构

```
custom/
├── database/my-pkg/          # 自定义包（结构同官方：app.yaml + bin.sh + uninstall.sh）
├── application/...
├── webserver/...
├── library/...
└── scripts/
    └── {username}/           # 按用户隔离的自定义脚本
        └── my-script.sh      # 可在 AppStore → 脚本管理 中编辑/运行
```

## 规则

- 自定义包与官方包**同名**时，以自定义包为准（优先显示、优先安装）
- 脚本编辑仅允许在此目录内（`custom/`），路径穿越会被拒绝
- 普通用户只能访问 `scripts/{自己的用户名}/`；admin 可访问全部
