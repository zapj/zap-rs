<template>
  <ServiceConfPage
    service="mysql"
    label="MySQL"
    desc="MySQL 服务状态与 my.cnf 配置（由应用商店安装后可用）。关键项写入 [mysqld] 托管区，保存后需「重启」mysql 服务生效。"
    install-hint="安装 MySQL 应用后，本页将自动显示版本、运行状态与 my.cnf 编辑入口。"
  />
</template>

<script setup lang="ts">
import ServiceConfPage from '../shared/ServiceConfPage.vue'
</script>
