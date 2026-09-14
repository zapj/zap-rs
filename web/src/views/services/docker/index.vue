<template>
  <ServiceConfPage
    service="docker"
    label="Docker"
    :desc="t('servicesDocker.desc')"
    :install-hint="t('servicesDocker.installHint')"
  />
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import ServiceConfPage from '../shared/ServiceConfPage.vue'

const { t } = useI18n()
</script>
