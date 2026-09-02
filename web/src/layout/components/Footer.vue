<template>
  <div class="footer">
    <div class="version-info">
      ZAP - v{{ APP_VERSION }} Build Date: #{{ BUILD_TIME }} @ copyright {{ new Date().getFullYear() }} 
    </div>
  </div>
</template>

<script setup lang="ts">

const BUILD_TIME = import.meta.env.VITE_BUILD_TIME
const APP_VERSION = import.meta.env.VITE_APP_VERSION
</script>

<style scoped>
.footer {
  padding: 15px 0;
  text-align: center;
  color: #666;
  font-size: 14px;
}

.version-info {
  margin: 0;
}
</style>