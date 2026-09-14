<template>
  <ServiceConfPage
    service="mysql"
    label="MySQL / MariaDB"
    :desc="t('servicesMysql.desc')"
    :install-hint="t('servicesMysql.installHint')"
  />
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import ServiceConfPage from '../shared/ServiceConfPage.vue'

const { t } = useI18n()
</script>
