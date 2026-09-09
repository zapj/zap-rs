<template>
  <ServiceConfPage
    service="mysql"
    label="MySQL / MariaDB"
    desc="MySQL 与 MariaDB 二合一的数据库服务配置：两者一次只能安装其一（共用 /usr/local/mysql 软链与 mysql.service），页面根据实际安装的引擎自动识别；关键项写入 [mysqld] 托管区。安装目录 / 主配置 / systemd 单元见下方详情。"
    install-hint="通过应用商店安装 MySQL 或 MariaDB 应用（二选一）后，本页将自动识别当前引擎，并显示版本、运行状态与 my.cnf 编辑入口。"
  />
</template>

<script setup lang="ts">
import ServiceConfPage from '../shared/ServiceConfPage.vue'
</script>
