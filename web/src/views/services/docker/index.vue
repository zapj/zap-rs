<template>
  <ServiceConfPage
    service="docker"
    label="Docker"
    desc="Docker 服务状态与 daemon.json 配置（由应用商店安装后可用）。daemon.json 不存在时可直接在此新建；保存会自动校验 JSON 语法。"
    install-hint="安装 Docker 应用后，本页将自动显示版本、运行状态与 daemon.json 编辑入口。"
  />
</template>

<script setup lang="ts">
import ServiceConfPage from '../shared/ServiceConfPage.vue'
</script>
