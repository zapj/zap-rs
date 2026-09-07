<template>
  <ServiceConfPage
    service="php"
    label="PHP"
    desc="PHP-FPM 运行参数与 php.ini 配置（由应用商店安装后可用）。保存关键配置后请「重载 / 重启」php-fpm 生效。"
    install-hint="安装 PHP 应用后，本页将自动显示 PHP 版本、运行状态与 php.ini 编辑入口。"
  />
</template>

<script setup lang="ts">
import ServiceConfPage from '../shared/ServiceConfPage.vue'
</script>
