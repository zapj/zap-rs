<template>
  <ServiceConfPage
    service="mysql"
    label="MySQL / MariaDB"
    desc="MySQL 与 MariaDB 二合一的数据库服务配置。两者一次只能安装其一（共用 /usr/local/mysql 与 mysql.service，配置同为 /etc/mysql/my.cnf），页面会根据实际安装的引擎自动识别。关键项写入 [mysqld] 托管区，保存后需「重启」服务生效。"
    install-hint="通过应用商店安装 MySQL 或 MariaDB 应用（二选一）后，本页将自动识别当前引擎，并显示版本、运行状态与 my.cnf 编辑入口。"
  />
</template>

<script setup lang="ts">
import ServiceConfPage from '../shared/ServiceConfPage.vue'
</script>
