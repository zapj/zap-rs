<template>
  <ServiceConfPage
    service="mariadb"
    label="MariaDB"
    desc="MariaDB 服务状态与配置（由应用商店安装后可用）。关键项写入 [mysqld] 托管区，保存后需「重启」mariadb 服务生效。"
    install-hint="安装 MariaDB 应用后，本页将自动显示版本、运行状态与配置文件编辑入口。"
  />
</template>

<script setup lang="ts">
import ServiceConfPage from '../shared/ServiceConfPage.vue'
</script>
