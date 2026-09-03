<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { EnvConf, EnvData } from '@/api/serverEnv'
import { getServerEnv, refreshServerEnv, saveServerEnvDefaults } from '@/api/serverEnv'

const env = ref<EnvData | null>(null)
const loading = ref(false)
const refreshing = ref(false)
const dialogVisible = ref(false)
const saving = ref(false)

const form = reactive<EnvConf>({
  webserver: '',
  php_default: '',
  database: '',
  vhost_mode: 'www',
  fpm_pool_defaults: '',
})

/** fpm pool 默认规格 —— 数值字段 */
const fpmNum = reactive({
  max_children: 10,
  start_servers: 3,
  min_spare_servers: 2,
  max_spare_servers: 5,
  max_requests: 1000,
  request_terminate_timeout: 300,
  max_execution_time: 300,
})
/** fpm pool 默认规格 —— 字符串字段 */
const fpmStr = reactive({
  pm: 'dynamic',
  memory_limit: '256M',
  post_max_size: '128M',
  upload_max_filesize: '128M',
})
const FPM_NUM_DEFAULTS: Record<string, number> = { ...fpmNum }
const FPM_STR_DEFAULTS: Record<string, string> = { ...fpmStr }

function fmtTime(ts?: number): string {
  if (!ts) return '--'
  const d = new Date(ts * 1000)
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`
}

const payload = computed(() => env.value?.payload ?? null)
const conf = computed(() => env.value?.conf ?? null)

const phpOptions = computed<string[]>(() => {
  const list = payload.value?.php?.instances ?? []
  const arr = list.map(i => shortOf(i.version)).filter(Boolean)
  return [...new Set(arr)]
})
function shortOf(v: string): string {
  return v.split('.').slice(0, 2).join('.')
}

const dbOptions = computed<string[]>(() => {
  const list = payload.value?.databases ?? []
  const names = list.map(d => d.name)
  const common = ['mysql', 'mariadb', 'postgresql', 'redis', 'mongodb']
  return [...new Set([...names, ...common])]
})

async function loadEnv() {
  loading.value = true
  try {
    const res = await getServerEnv()
    env.value = res.data
  } catch {
    /* 拦截器已提示 */
  } finally {
    loading.value = false
  }
}

async function refresh() {
  refreshing.value = true
  try {
    const res = await refreshServerEnv()
    env.value = res.data
    ElMessage.success(res.message || '运行环境已刷新')
  } catch {
    /* 拦截器已提示 */
  } finally {
    refreshing.value = false
  }
}

function openDefaultsDialog() {
  const c = conf.value
  form.webserver = c?.webserver ?? ''
  form.php_default = c?.php_default ?? ''
  form.database = c?.database ?? ''
  form.vhost_mode = c?.vhost_mode === 'system' ? 'system' : 'www'
  // 回填 fpm 默认规格（先重置再覆盖）
  resetFpmForm()
  const raw = c?.fpm_pool_defaults
  if (raw) {
    try {
      const obj = JSON.parse(raw) as Record<string, unknown>
      Object.keys(fpmNum).forEach(k => {
        const v = obj[k]
        const n = Number(v)
        if (v !== undefined && v !== null && Number.isFinite(n)) fpmNum[k as keyof typeof fpmNum] = n
      })
      Object.keys(fpmStr).forEach(k => {
        const v = obj[k]
        if (v !== undefined && v !== null) fpmStr[k as keyof typeof fpmStr] = String(v)
      })
    } catch {
      /* 非法 JSON 忽略，使用默认 */
    }
  }
  dialogVisible.value = true
}

function resetFpmForm() {
  Object.keys(fpmNum).forEach(k => {
    fpmNum[k as keyof typeof fpmNum] = FPM_NUM_DEFAULTS[k]
  })
  Object.keys(fpmStr).forEach(k => {
    fpmStr[k as keyof typeof fpmStr] = FPM_STR_DEFAULTS[k]
  })
}

function fpmSpecJson(): string {
  return JSON.stringify({ ...fpmStr, ...fpmNum })
}

async function saveDefaults() {
  const prevMode = conf.value?.vhost_mode ?? 'www'
  const nextMode = form.vhost_mode
  if (prevMode !== nextMode) {
    const tip =
      nextMode === 'system'
        ? '切换到「独立系统用户」后：\n· 新用户创建/同步时会自动 useradd（nologin）并把 web 目录归该账号；\n· 存量站点请到「虚拟主机 → 全部再同步」按新模式重建（自动生成每用户 PHP-FPM pool）。'
        : '切换到「统一 www 用户」后：\n· 站点同步时 web 目录属主与 PHP pool 会回到 www / 全局实例；\n· 此前已创建的 Linux 系统账号与专属 pool 不会被自动删除（保留为孤儿账号），如不再使用请手动清理。'
    try {
      await ElMessageBox.confirm(
        `${tip}\n\n是否继续保存？`,
        '切换虚拟主机运行模式',
        { type: 'warning', confirmButtonText: '保存并切换' }
      )
    } catch {
      return
    }
  }
  saving.value = true
  try {
    const res = await saveServerEnvDefaults({
      webserver: form.webserver,
      php_default: form.php_default,
      database: form.database,
      vhost_mode: form.vhost_mode,
      fpm_pool_defaults: fpmSpecJson(),
    })
    ElMessage.success(res.message || '默认配置已保存')
    dialogVisible.value = false
    loadEnv()
  } catch {
    /* 拦截器已提示 */
  } finally {
    saving.value = false
  }
}

onMounted(loadEnv)
</script>

<template>
  <div class="env-container">
    <el-card shadow="never" v-loading="loading">
      <template #header>
        <div class="card-header">
          <span>服务器运行环境</span>
          <div class="header-actions">
            <el-tag v-if="env?.refreshed" size="small" type="success" style="margin-right: 8px">
              已自动刷新
            </el-tag>
            <span class="detected-at" v-if="payload">检测于 {{ fmtTime(env?.detected_at) }}</span>
            <el-button type="primary" size="small" :loading="refreshing" @click="refresh">
              重新检测
            </el-button>
            <el-button size="small" @click="openDefaultsDialog">默认配置</el-button>
          </div>
        </div>
      </template>

      <el-alert
        v-if="env?.error"
        :title="`自动探测暂不可用（${env.error}），当前展示上次缓存快照。可稍后手动重新检测。`"
        type="warning"
        :closable="false"
        show-icon
        style="margin-bottom: 16px"
      />

      <template v-if="payload">
        <!-- 操作系统 -->
        <el-descriptions title="操作系统" :column="2" border class="env-section">
          <el-descriptions-item label="主机名">{{ payload.hostname || '--' }}</el-descriptions-item>
          <el-descriptions-item label="系统">
            {{ payload.os.name }} {{ payload.os.version }}
          </el-descriptions-item>
          <el-descriptions-item label="内核">{{ payload.os.kernel || '--' }}</el-descriptions-item>
          <el-descriptions-item label="架构">{{ payload.os.arch || '--' }}</el-descriptions-item>
        </el-descriptions>

        <!-- Web 服务器 -->
        <el-descriptions title="Web 服务器" :column="1" border class="env-section">
          <el-descriptions-item label="类型">
            <template v-if="payload.webserver?.flavor && payload.webserver.flavor !== 'none'">
              <el-tag :type="payload.webserver.flavor === 'openresty' ? 'warning' : 'success'" size="small">
                {{ payload.webserver.flavor }}
              </el-tag>
              <el-tag size="small" style="margin-left: 8px">v{{ payload.webserver.version || '--' }}</el-tag>
              <el-tag
                size="small"
                :type="payload.webserver.running ? 'success' : 'info'"
                style="margin-left: 8px"
              >
                {{ payload.webserver.running ? '运行中' : '未运行' }}
              </el-tag>
            </template>
            <el-tag v-else size="small" type="info">未检测到 Nginx / OpenResty</el-tag>
          </el-descriptions-item>
          <el-descriptions-item v-if="payload.webserver?.binary" label="可执行文件">
            {{ payload.webserver.binary }}
          </el-descriptions-item>
          <el-descriptions-item v-if="payload.webserver?.conf" label="主配置">
            {{ payload.webserver.conf }}
          </el-descriptions-item>
          <el-descriptions-item v-if="payload.webserver?.sites_dir" label="站点配置目录">
            {{ payload.webserver.sites_dir }}
          </el-descriptions-item>
        </el-descriptions>

        <!-- PHP -->
        <div class="env-section">
          <div class="section-title">
            PHP
            <el-tag v-if="payload.php?.default" size="small" type="primary" style="margin-left: 8px">
              默认 {{ payload.php.default }}
            </el-tag>
          </div>
          <el-table :data="payload.php?.instances ?? []" size="small" border style="margin-top: 8px">
            <el-table-column label="版本" width="110">
              <template #default="{ row }">
                <el-tag v-if="row.default" type="primary" size="small">{{ row.version }}</el-tag>
                <span v-else>{{ row.version }}</span>
              </template>
            </el-table-column>
            <el-table-column prop="binary" label="可执行文件" show-overflow-tooltip />
            <el-table-column prop="socket" label="FPM Socket" show-overflow-tooltip>
              <template #default="{ row }">{{ row.socket || '--' }}</template>
            </el-table-column>
            <el-table-column label="状态" width="90">
              <template #default="{ row }">
                <el-tag :type="row.running ? 'success' : 'info'" size="small">
                  {{ row.running ? '运行中' : '未运行' }}
                </el-tag>
              </template>
            </el-table-column>
          </el-table>
        </div>

        <!-- 数据库 -->
        <div class="env-section">
          <div class="section-title">数据库</div>
          <el-table :data="payload.databases ?? []" size="small" border style="margin-top: 8px">
            <el-table-column label="实例" width="160">
              <template #default="{ row }">
                <el-tag size="small">{{ row.name }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="version" label="版本" />
            <el-table-column label="状态" width="110">
              <template #default="{ row }">
                <el-tag :type="row.running ? 'success' : 'info'" size="small">
                  {{ row.running ? '运行中' : '未运行' }}
                </el-tag>
              </template>
            </el-table-column>
          </el-table>
        </div>

        <!-- 工具链 -->
        <div class="env-section">
          <div class="section-title">常用工具</div>
          <el-table :data="payload.tools ?? []" size="small" border style="margin-top: 8px">
            <el-table-column label="名称" width="160">
              <template #default="{ row }">
                <el-tag size="small" type="info">{{ row.name }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="version" label="版本" />
          </el-table>
        </div>
      </template>

      <el-empty v-else description="暂无运行环境数据，请点击右上角「重新检测」" :image-size="80" />
    </el-card>

    <!-- 全局默认配置 -->
    <el-dialog v-model="dialogVisible" title="全局默认配置" width="640px">
      <el-form label-width="130px">
        <el-form-item label="默认 Web 服务器">
          <el-select v-model="form.webserver" clearable placeholder="跟随自动探测" style="width: 100%">
            <el-option label="跟随自动探测（自动）" value="" />
            <el-option label="nginx" value="nginx" />
            <el-option label="openresty" value="openresty" />
          </el-select>
        </el-form-item>
        <el-form-item label="默认 PHP 版本">
          <el-select
            v-model="form.php_default"
            clearable
            filterable
            allow-create
            default-first-option
            placeholder="不指定（站点可单独选择）"
            style="width: 100%"
          >
            <el-option v-for="v in phpOptions" :key="v" :label="v" :value="v" />
          </el-select>
          <div class="form-tip">新建站点/部署时的默认 PHP 版本预选（如 8.3 / php83）</div>
        </el-form-item>
        <el-form-item label="默认数据库">
          <el-select
            v-model="form.database"
            clearable
            filterable
            allow-create
            default-first-option
            placeholder="不指定"
            style="width: 100%"
          >
            <el-option v-for="d in dbOptions" :key="d" :label="d" :value="d" />
          </el-select>
        </el-form-item>

        <el-divider content-position="left">虚拟主机运行模式</el-divider>
        <el-form-item label="运行模式">
          <el-radio-group v-model="form.vhost_mode">
            <el-radio value="www">
              统一 www 用户
              <span class="mode-sub">所有站点文件与 PHP 均以 www 运行，简单易维护</span>
            </el-radio>
            <el-radio value="system">
              独立系统用户
              <span class="mode-sub">每个面板用户对应一个 Linux 账号，PHP-FPM 以该账号运行</span>
            </el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item label=" ">
          <el-alert
            :title="form.vhost_mode === 'system'
              ? '切换后：新用户创建时将自动 useradd（nologin）并 chown 家目录；存量用户请到「用户管理 → 同步家目录/运行实体」补齐，站点同步时自动生成独立 PHP-FPM pool（每用户每 PHP 版本一个）。'
              : '统一 www 模式：站点文件与 PHP-FPM 均归 www 用户，站点使用全局 PHP socket。'"
            type="info"
            :closable="false"
            show-icon
          />
        </el-form-item>

        <el-divider content-position="left">PHP-FPM 默认 pool 规格</el-divider>
        <el-form-item label="进程管理模式">
          <el-radio-group v-model="fpmStr.pm">
            <el-radio value="dynamic">dynamic（动态）</el-radio>
            <el-radio value="static">static（固定）</el-radio>
            <el-radio value="ondemand">ondemand（按需）</el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item v-if="fpmStr.pm !== 'ondemand'" label="最大子进程数">
          <el-input-number v-model="fpmNum.max_children" :min="1" :max="512" controls-position="right" />
          <div class="form-tip">pm.max_children：常驻 worker 上限（建议 = 可用内存 MB ÷ 单进程约 50-100MB）</div>
        </el-form-item>
        <el-form-item v-if="fpmStr.pm === 'dynamic'" label="启动子进程数">
          <el-input-number v-model="fpmNum.start_servers" :min="1" :max="128" controls-position="right" />
        </el-form-item>
        <el-form-item v-if="fpmStr.pm === 'dynamic'" label="空闲下限 / 上限">
          <el-input-number v-model="fpmNum.min_spare_servers" :min="1" :max="128" controls-position="right" />
          <span style="margin: 0 8px; color: #909399">~</span>
          <el-input-number v-model="fpmNum.max_spare_servers" :min="1" :max="256" controls-position="right" />
        </el-form-item>
        <el-form-item label="单进程最大请求数">
          <el-input-number v-model="fpmNum.max_requests" :min="0" :max="100000" controls-position="right" />
          <div class="form-tip">pm.max_requests：达到后自动回收（0 = 不回收），防内存泄漏</div>
        </el-form-item>
        <el-form-item label="请求超时(秒)">
          <el-input-number v-model="fpmNum.request_terminate_timeout" :min="1" :max="86400" controls-position="right" />
        </el-form-item>
        <el-form-item label="内存限制">
          <el-select v-model="fpmStr.memory_limit" filterable allow-create default-first-option style="width: 180px">
            <el-option v-for="m in ['128M', '256M', '512M', '1G', '2G']" :key="m" :label="m" :value="m" />
          </el-select>
        </el-form-item>
        <el-form-item label="上传大小上限">
          <el-select v-model="fpmStr.upload_max_filesize" filterable allow-create default-first-option style="width: 180px">
            <el-option v-for="m in ['64M', '128M', '256M', '512M', '1G']" :key="m" :label="m" :value="m" />
          </el-select>
        </el-form-item>
        <el-form-item label="POST 大小上限">
          <el-select v-model="fpmStr.post_max_size" filterable allow-create default-first-option style="width: 180px">
            <el-option v-for="m in ['64M', '128M', '256M', '512M', '1G']" :key="m" :label="m" :value="m" />
          </el-select>
        </el-form-item>
        <el-form-item label="最大执行时间(秒)">
          <el-input-number v-model="fpmNum.max_execution_time" :min="1" :max="86400" controls-position="right" />
        </el-form-item>
        <el-form-item label=" ">
          <el-button size="small" @click="resetFpmForm">恢复默认规格</el-button>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="saving" @click="saveDefaults">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.env-container {
  padding: 20px;
}
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.header-actions {
  display: flex;
  align-items: center;
}
.detected-at {
  color: #909399;
  font-size: 12px;
  margin-right: 12px;
}
.env-section {
  margin-top: 20px;
}
.section-title {
  font-weight: 600;
  color: #303133;
  display: flex;
  align-items: center;
}
.form-tip {
  color: #909399;
  font-size: 12px;
  line-height: 1.6;
}
.mode-sub {
  display: block;
  font-size: 12px;
  color: #909399;
  line-height: 1.5;
}
</style>
