<template>
  <div class="app-script-guide">
    <el-card shadow="never">
      <template #header>
        <div class="card-header">
          <div class="header-left">
            <span class="title">应用脚本编写指南</span>
            <el-tag type="info" size="small" style="margin-left: 8px">开发</el-tag>
          </div>
        </div>
      </template>

      <div class="guide-body">
        <aside class="guide-toc">
          <div class="toc-title">本页目录</div>
          <a v-for="t in toc" :key="t.id" :href="'#' + t.id" class="toc-item">{{ t.label }}</a>
        </aside>

        <main class="guide-content">
          <p class="lead">
            为应用商店仓库编写包描述（<code>app.yaml</code>）与生命周期脚本（安装 / 卸载 /
            升级）的完整约定。内容与
            <code>zapexec</code> / <code>zapd</code> 实现保持同步，脚本作者请以此为准。
          </p>

          <!-- 一、包结构 -->
          <h2 id="sec-package">一、包结构与目录约定</h2>
          <p>仓库内每个应用是一个 <code>category/name</code> 目录（分类/名称），包路径仅允许 ASCII 字母、数字、<code>-</code>、<code>_</code>：</p>
          <pre class="code">{{ codes.tree }}</pre>
          <ul>
            <li><code>app.yaml</code>：应用描述，见第二节；</li>
            <li><code>bin.sh</code> / <code>uninstall.sh</code> / <code>upgrade.sh</code>：默认生命周期脚本文件名，可用 <code>scripts</code> 字段覆盖（见第三节）；</li>
            <li>其余文件（源码、配置模板、编译资源等）随包下发，运行时位于快照目录内，由脚本自行引用。</li>
          </ul>
          <el-alert type="warning" :closable="false" class="doc-tip">
            脚本实际执行的是「本次运行的快照副本」，而不是仓库源目录里的同名文件，详见第四节「执行模型」。
          </el-alert>

          <!-- 二、app.yaml -->
          <h2 id="sec-appyaml">二、app.yaml 字段说明</h2>
          <table class="doc-table">
            <thead>
              <tr><th style="width: 180px">字段</th><th style="width: 120px">类型</th><th>说明</th></tr>
            </thead>
            <tbody>
              <tr><td><code>name</code></td><td>string</td><td>包名，缺省取目录名</td></tr>
              <tr><td><code>title</code></td><td>string</td><td>显示名称</td></tr>
              <tr><td><code>category</code></td><td>string</td><td>分类，缺省取父目录名</td></tr>
              <tr><td><code>description</code></td><td>string</td><td>简介</td></tr>
              <tr><td><code>version</code></td><td>string / array</td><td>单值或数组（如 <code>[1.24.0, 1.22.1]</code>）；数组表示支持安装的多个版本，首个为默认版本</td></tr>
              <tr><td><code>deps</code></td><td>string[]</td><td>兼容旧写法：依赖名列表</td></tr>
              <tr><td><code>dependencies</code></td><td>map</td><td>依赖库名 → 版本要求（如 <code>openssl: 1.1.1w</code>）</td></tr>
              <tr><td><code>actions</code></td><td>map</td><td>自定义操作按钮：动作键 → 文案（如 <code>build: 编译安装</code>）。发起安装/升级时 env 注入 <code>ACTION=动作键</code>，选项按动作键区分（见第六节）</td></tr>
              <tr><td><code>scripts</code></td><td>map</td><td>脚本文件名覆盖：<code>install / uninstall / upgrade</code> → 文件名</td></tr>
              <tr><td><code>options</code></td><td>map / list</td><td>安装/升级可选项定义：动作键 → 选项列表；顶层直接写列表等价于作用于 install 动作（见第六节）</td></tr>
              <tr><td><code>allow_multiple_instances</code></td><td>bool</td><td>为 true 时已安装仍可再装其它版本（多实例）</td></tr>
              <tr><td><code>default_port</code></td><td>int</td><td>默认端口（仅展示用途）</td></tr>
            </tbody>
          </table>

          <!-- 三、脚本与生命周期 -->
          <h2 id="sec-lifecycle">三、生命周期脚本与升级策略</h2>
          <table class="doc-table">
            <thead>
              <tr><th>流程</th><th style="width: 200px">脚本解析</th><th>说明</th></tr>
            </thead>
            <tbody>
              <tr>
                <td>安装</td>
                <td><code>scripts.install</code>，缺省 <code>bin.sh</code></td>
                <td>包未安装时执行；成功后系统在 <code>APP_PATH</code> 写入运行元数据 <code>meta.yaml</code>（版本 / 来源 / 安装时间 / run_id），脚本须自行登记实例信息 <code>info.yaml</code>（见下「实例登记」）</td>
              </tr>
              <tr>
                <td>卸载</td>
                <td><code>scripts.uninstall</code>，缺省 <code>uninstall.sh</code></td>
                <td>仅已安装时可执行；成功后系统自动删除 <code>APP_PATH</code>（含 <code>meta.yaml</code> / <code>info.yaml</code>），需要保留的备份请在脚本内自行处理</td>
              </tr>
              <tr>
                <td>升级</td>
                <td>存在 <code>upgrade.sh</code> 则执行；否则自动回退为「先 <code>uninstall.sh</code>、后 <code>bin.sh</code>」两段</td>
                <td>旧版本目录清理由脚本自理（<code>APP_OLD_VERSION</code> 携带旧版本号）；两段式策略中卸载阶段请勿删除还需复用的数据。成功后系统刷新 <code>meta.yaml</code>；若安装目录 / 服务名有变，脚本应同步更新 <code>info.yaml</code></td>
              </tr>
            </tbody>
          </table>

          <p class="sec-sub"><strong>实例登记（info.yaml）</strong>：<code>APP_PATH</code>（<code>$ZAP_PATH/data/apps/&lt;category&gt;/&lt;name&gt;/</code>）下两类记录分工——<code>meta.yaml</code> 由系统在成功时写入（版本 / 来源 / 安装时间 / run_id 等运行元数据，脚本不要写）；<code>info.yaml</code> 由安装 / 升级脚本在结束前自行登记，Web 端「已安装」据此展示实例、探测状态并支持启停：</p>
          <pre class="code">{{ codes.infoYaml }}</pre>
          <table class="doc-table">
            <thead>
              <tr><th style="width: 140px">字段</th><th style="width: 110px">类型</th><th>说明</th></tr>
            </thead>
            <tbody>
              <tr><td><code>svc_name</code></td><td>string</td><td>守护型应用填 systemd unit 名（如 <code>mysql</code> / <code>nginx</code> / <code>php-fpm-85</code>），状态探测与面板启停走 systemctl</td></tr>
              <tr><td><code>instance</code></td><td>string</td><td>实例展示标识（如 <code>php85</code>、<code>openssl1011</code>）</td></tr>
              <tr><td><code>install_dir</code></td><td>string</td><td>软件本体实际安装目录（位于 <code>$APPS_DIR</code> 下），日志定位 / 「打开目录」用</td></tr>
              <tr><td><code>config_file</code></td><td>string</td><td>主配置文件绝对路径</td></tr>
              <tr><td><code>pid_file</code></td><td>string</td><td>pid 文件路径；守护型填写，作无 systemd 环境下的兜底探活</td></tr>
              <tr><td><code>expose</code></td><td>string / string[]</td><td>暴露入口：<code>tcp:80</code>、<code>unix:/run/xxx.sock</code> 等，可多行数组；无则 <code>none</code></td></tr>
              <tr><td><code>tags</code></td><td>string[]</td><td>分类 / 特性标签（如 <code>webserver</code>、<code>library</code>）</td></tr>
            </tbody>
          </table>
          <el-alert type="info" :closable="false" class="doc-tip">
            无守护进程的库类（如 openssl / libpng / libpcre2）不写 <code>svc_name</code> / <code>pid_file</code>，状态由系统返回 unknown；需面板支持「启动 / 停止 / 状态」的守护型应用务必登记 <code>svc_name</code>。
          </el-alert>

          <ul>
            <li><strong>退出码约定</strong>：任一步骤退出码非 0 即视为失败并中断后续步骤，任务最终以最后一次非 0 退出码结束；</li>
            <li><strong>输出</strong>：脚本 <code>stdout / stderr</code> 实时追加进 <code>run-&lt;run_id&gt;.log</code>，Web 端可跟踪，失败排查请把原因打印到输出；</li>
            <li>脚本以 root 运行，但环境是「清空 + 安全白名单 PATH」的纯净环境（见第五节），不要依赖宿主机自定义变量。</li>
          </ul>

          <!-- 四、执行模型 -->
          <h2 id="sec-model">四、执行模型：快照副本 + 失败保留重跑</h2>
          <p>每次 install / uninstall / upgrade 启动前，系统把仓库中该包目录整体复制为本次运行的脚本快照：</p>
          <pre class="code">{{ codes.model }}</pre>
          <ul>
            <li><code>PKG_PATH</code> 指向的是这个快照目录（含 <code>app.yaml</code>、脚本、<code>options.env</code> 等），不是仓库源目录；运行中修改仓库不会影响已在队列/运行中的任务；</li>
            <li><code>run.json</code> 记录本次运行的原始参数（动作 / 版本 / 选项），供重跑还原环境；</li>
            <li>全部步骤退出码为 0（成功）后，系统自动清理 <code>runs/&lt;run_id&gt;</code>（脚本快照与 <code>build/</code> 编译目录一并清理），避免磁盘堆积；</li>
            <li>失败（任一退出码非 0）则保留整个运行现场：<code>pkg/</code> 内脚本与 <code>options.env</code> 可在「应用商店 → 运行记录」中读取/编辑，<code>build/</code> 编译残留一并保留便于排查，之后「编辑脚本 / 重跑」复用同一快照重试；</li>
            <li>全局串行队列：同一时间仅执行一个脚本任务，后续任务先排队，日志中会提示「任务进入执行队列，等待前序任务完成后自动开始」；</li>
            <li>日志结束时追加一行 <code>__ZAP_DONE__ &lt;退出码&gt;</code> 作为完成标记。</li>
          </ul>

          <!-- 五、环境变量 -->
          <h2 id="sec-env">五、注入的环境变量</h2>
          <p>子进程通过 <code>env_clear()</code> 清空宿主机环境，仅注入白名单 <code>PATH</code> 与下表变量（脚本可放心使用，不会被污染）：</p>
          <table class="doc-table">
            <thead>
              <tr><th style="width: 200px">变量</th><th>含义</th></tr>
            </thead>
            <tbody>
              <tr><td><code>PATH</code></td><td>安全白名单：/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin（不可覆盖）</td></tr>
              <tr><td><code>ZAP_PATH</code></td><td>面板安装根目录，缺省 /usr/local/zap</td></tr>
              <tr><td><code>ZAPCTL</code></td><td>zapctl 可执行文件路径（<code>$ZAP_PATH/zapctl</code>）</td></tr>
              <tr><td><code>APPS_DIR</code></td><td>软件本体安装根目录（默认 <code>/usr/local/apps</code>；zapd / zapexec 启动时可经 <code>ZAP_APPS_DIR</code> 覆盖）。<code>configure --prefix</code> 等最终安装目标以此作基准拼接（如 <code>$APPS_DIR/nginx-1.24.0</code>），不要硬编码系统目录</td></tr>
              <tr><td><code>LOG_FILE</code></td><td>本次运行日志绝对路径（<code>data/appstore/logs/run-&lt;run_id&gt;.log</code>）</td></tr>
              <tr><td><code>CPU_NUM</code></td><td>可用 CPU 核数（编译可参考，如 make -j）</td></tr>
              <tr><td><code>PKG_PATH</code></td><td>本次运行脚本快照目录（含 app.yaml / 脚本 / options.env / options.json）</td></tr>
              <tr><td><code>PKG_SRC_PATH</code></td><td>仓库内源码目录（<code>repos/&lt;repo&gt;/&lt;category&gt;/&lt;name&gt;</code>），需要读源码附件时用</td></tr>
              <tr><td><code>APP_ID</code></td><td>本次运行 run_id</td></tr>
              <tr><td><code>APP_NAME</code></td><td>包名</td></tr>
              <tr><td><code>APP_PATH</code></td><td>本应用元数据登记目录（<code>$ZAP_PATH/data/apps/&lt;category&gt;/&lt;name&gt;</code>）：系统写 <code>meta.yaml</code>、脚本登记 <code>info.yaml</code>，勿放安装产物（登记字段见第三节「实例登记」）</td></tr>
              <tr><td><code>BUILD_PATH</code></td><td>本次运行专属编译目录（<code>$ZAP_PATH/data/appstore/runs/&lt;run_id&gt;/build</code>），编译中间产物放这里；脚本开头可放心 <code>rm -rf</code>——路径按 run 隔离，成功后随运行现场一并清理、失败保留供排查</td></tr>
              <tr><td><code>ZAP_DATA_PATH</code></td><td>面板数据目录（<code>$ZAP_PATH/data</code>）</td></tr>
              <tr><td><code>APP_VERSION</code></td><td>本次安装/升级的目标版本</td></tr>
              <tr><td><code>MAJOR_VERSION</code></td><td>目标版本主版本号（如 1.24.0 → 1）</td></tr>
              <tr><td><code>MINOR_VERSION</code></td><td>目标版本次版本号（如 1.24.0 → 24）</td></tr>
              <tr><td><code>APP_OLD_VERSION</code></td><td>升级前旧版本（仅升级注入）</td></tr>
              <tr><td><code>ACTION</code></td><td>动作键（由 actions 自定义操作发起时注入，如 build）</td></tr>
              <tr><td>选项变量</td><td>每个 options 项按 <code>name</code> 直接注入同名环境变量（见第七节）</td></tr>
            </tbody>
          </table>

          <!-- 六、options 定义 -->
          <h2 id="sec-options">六、options：安装 / 升级可选项</h2>
          <p>
            <code>app.yaml</code> 顶层 <code>options</code> 定义表单项；Web 端检测到选项时，点击安装 / 升级会先弹出选项表单，确认后选项随安装请求提交。
            结构为「动作键 → 选项列表」，顶层直接写列表等价于作用于 install 动作：
          </p>
          <pre class="code">{{ codes.optionsYaml }}</pre>
          <table class="doc-table">
            <thead>
              <tr><th style="width: 160px">字段</th><th style="width: 120px">类型</th><th>说明</th></tr>
            </thead>
            <tbody>
              <tr><td><code>name</code></td><td>string</td><td>选项名，即注入的环境变量名 / options.env 键（命名规则见第八节）</td></tr>
              <tr><td><code>label</code></td><td>string</td><td>表单显示名</td></tr>
              <tr><td><code>type</code></td><td>string</td><td>控件类型：<code>string</code>（文本）/ <code>number</code>（数值）/ <code>bool</code>（开关）/ <code>select</code>（单选）/ <code>multiselect</code>（多选）；缺省 string</td></tr>
              <tr><td><code>default</code></td><td>any</td><td>缺省值</td></tr>
              <tr><td><code>required</code></td><td>bool</td><td>是否必填</td></tr>
              <tr><td><code>placeholder</code></td><td>string</td><td>string / select 输入框占位提示</td></tr>
              <tr><td><code>desc</code></td><td>string</td><td>字段下方说明文字</td></tr>
              <tr><td><code>choices</code></td><td>array</td><td>select / multiselect 候选：字符串，或 <code>{ label, value }</code> 对象（label 为显示名）</td></tr>
              <tr><td><code>separator</code></td><td>string</td><td>multiselect 提交时的拼接符，缺省为空格；候选值本身可能含空格时建议改用其它分隔符</td></tr>
            </tbody>
          </table>
          <p><strong>值归一化</strong>（提交后全部为标量字符串，无 JSON 对象）：</p>
          <ul>
            <li><code>bool</code> → <code>true</code> / <code>false</code>；</li>
            <li><code>number</code> → 数字字符串；</li>
            <li><code>multiselect</code> → 勾选项按 <code>separator</code> 拼接为一个字符串（缺省空格，如 <code>"ssl gzip stub_status"</code>）；</li>
            <li>安装与升级共用同一份选项定义与表单逻辑。</li>
          </ul>

          <!-- 七、脚本如何读取 -->
          <h2 id="sec-read">七、脚本如何读取选项（核心）</h2>
          <p>选项会随快照一起落盘，并与脚本放在同一个目录，脚本有三种等价读取方式，任选其一：</p>
          <pre class="code">{{ codes.readOpts }}</pre>
          <ul>
            <li><strong>环境变量（推荐）</strong>：每个选项按 <code>name</code> 直接注入子进程 env，脚本零成本使用 <code>$MODULES</code> 即可，无需 source；</li>
            <li><strong>options.env</strong>：快照目录内每行 <code>NAME='值'</code>（单引号转义，可安全 source），也便于在「编辑脚本 / 重跑」前人工查看与修改；</li>
            <li><strong>options.json</strong>：结构化 JSON 对象，适合 python / jq 等程序化处理。</li>
          </ul>
          <el-alert type="info" :closable="false" class="doc-tip">
            多选值默认以空格拼接。脚本中如需逐项遍历：<code>for m in $MODULES; do ...; done</code>；
            若包声明了 <code>separator: ','</code>，可先 <code>MODULES=${MODULES//,/ }</code> 再遍历。
          </el-alert>

          <!-- 八、校验与限制 -->
          <h2 id="sec-limits">八、选项命名与长度限制</h2>
          <table class="doc-table">
            <thead>
              <tr><th style="width: 220px">维度</th><th>限制</th></tr>
            </thead>
            <tbody>
              <tr><td>选项名</td><td>≤ 64 字符；首字符须英文字母或下划线；其余仅 ASCII 字母 / 数字 / 下划线</td></tr>
              <tr><td>保留前缀 / 名称</td><td>禁止 <code>ZAP_</code> / <code>PKG_</code> / <code>APP_</code> / <code>ACTION</code> / <code>SCRIPT_</code> / <code>RUN_</code> 前缀，禁止 <code>PATH</code> / <code>HOME</code></td></tr>
              <tr><td>单次数量</td><td>≤ 64 项</td></tr>
              <tr><td>单值长度</td><td>≤ 4096 字符（按 Unicode 字符数计，含多选拼接后的整体值）</td></tr>
            </tbody>
          </table>
          <p><strong>关于「env 长度」的系统层限制</strong>：选项值最终随单个环境变量经 <code>execve</code> 传给 bash，因此存在两层系统硬限制——单个环境变量串（<code>KEY=VALUE</code>）上限 <code>MAX_ARG_STRLEN</code> = 128KB，全部 env + 参数合计上限 <code>ARG_MAX</code>（通常 2MB，<code>getconf ARG_MAX</code> 可查）。超出会直接报 <code>E2BIG</code>。由于应用层已把单值限制在 4096 字符（全中文 ≈ 12KB、64 项全满也远低于 2MB），正常 Web 表单路径不会触达系统限制，实际瓶颈是 4096。</p>
          <el-alert type="warning" :closable="false" class="doc-tip">
            两点提醒：① 4096 按「字符」计数、execve 按「字节」计数，极端全 emoji（4 字节/字符）单选项也只有 16KB，安全；② 「编辑脚本 / 重跑」路径直接读取
            <code>options.env</code> 文件、不做 4096 长度校验，手工往文件里塞超长值（单行超过 128KB）会在启动 bash 时 <code>E2BIG</code> 失败，请勿这样做。
          </el-alert>

          <!-- 九、完整示例 -->
          <h2 id="sec-example">九、完整示例：nginx 编译模块多选</h2>
          <p>仓库样例 <code>webserver/nginx</code>（数据目录 <code>data/appstore/repos/zap-appstore/webserver/nginx/</code>）演示了「编译哪些模块」的多选场景。要点：</p>
          <ul>
            <li>动作键 <code>build</code> 与 <code>actions.build: 编译安装</code> 对应，从该动作发起安装时用户可勾选模块；</li>
            <li>选项脚本直接以 <code>$MODULES</code> / <code>$EXTRA_CONFIG</code> 取用（env 已注入）；</li>
            <li>多选缺省空格拼接，configure 尾部整体展开即得到模块参数。</li>
          </ul>
          <pre class="code">{{ codes.nginxYaml }}</pre>
          <pre class="code">{{ codes.nginxUse }}</pre>

          <!-- 十、失败排查 -->
          <h2 id="sec-trouble">十、失败排查与重跑工作流</h2>
          <ol>
            <li>「应用商店 → 运行记录」查看失败运行的日志（末尾 <code>__ZAP_DONE__ &lt;code&gt;</code> 即退出码）；</li>
            <li>失败现场默认保留在 <code>data/appstore/runs/&lt;run_id&gt;/</code>：<code>pkg/</code> 内脚本与 <code>options.env</code> 可查看/编辑，<code>build/</code> 编译残留一并保留供排查；然后「编辑脚本 / 重跑」——重跑以快照内文件为准，编辑过的选项同样生效；</li>
            <li>重跑成功后系统按新 run_id 重新跟踪日志；新运行成功会清理其快照，原失败快照由「重跑」发起时一并清理。</li>
          </ol>
          <p class="footnote">本文档与 <code>app.yaml</code> 解析、<code>zapexec/src/verbs/appstore.rs</code> 执行实现保持同步；如有出入以代码为准。</p>
        </main>
      </div>
    </el-card>
  </div>
</template>

<script setup lang="ts">
const codes = {
  tree: `repos/<仓库>/
└── <category>/
    └── <name>/
        ├── app.yaml          # 应用描述（含 scripts / options / actions）
        ├── bin.sh            # 安装脚本（缺省文件名）
        ├── uninstall.sh      # 卸载脚本（缺省文件名）
        ├── upgrade.sh        # 升级脚本（可选）
        └── ...               # 其余资源，随快照下发
`,
  model: `\$ZAP_PATH/
├── data/appstore/
│   ├── repos/<repo>/<category>/<name>/   # 仓库源（只读参考）
│   ├── runs/<run_id>/                    # 一次运行完整现场：成功后整体清理，失败保留
│   │   ├── pkg/                          # 脚本快照：app.yaml + 脚本 + options.env/json（可编辑重跑）
│   │   │   ├── app.yaml
│   │   │   ├── bin.sh
│   │   │   ├── options.env               # 安装/升级选项（可 source、可编辑）
│   │   │   └── options.json
│   │   ├── build/                        # 编译目录（BUILD_PATH，随 run 一并清理）
│   │   └── run.json                      # 运行参数记录
│   └── logs/run-<run_id>.log             # 实时日志（结束含 __ZAP_DONE__ <code>）
├── data/apps/<category>/<name>/          # 安装元数据：meta.yaml（系统写）+ info.yaml（脚本写）
└── \$APPS_DIR/<name>-<ver>/              # 软件本体：默认 /usr/local/apps（ZAP_APPS_DIR 可覆盖）
`,
  infoYaml: `# 安装脚本末尾：登记实例信息（值用脚本内变量展开，勿字面写死）
cat > "\${APP_PATH}/info.yaml" <<EOF
svc_name: php-fpm-\${PHP_SHORT_VERSION}   # 守护型填 systemd unit 名；库类删除此行
instance: php\${PHP_SHORT_VERSION}
install_dir: \${PHP_INSTALL_PATH}          # 软件本体在 \$APPS_DIR 下的实际安装目录
config_file: \${PHP_INSTALL_PATH}/etc/php.ini
pid_file: \${PHP_FPM_PID}                  # 无守护进程的库类删除此行
expose: unix:\${PHP_FPM_SOCK}
tags:
  - language
EOF
`,
  optionsYaml: `options:
  build:                # 动作键；顶层直接写列表 = install
    - name: MODULES
      label: 编译模块
      type: multiselect
      choices: [ssl, gzip, stub_status, ipv6]
      separator: ' '
      default: ssl
      required: true
      desc: 勾选需要编译进 nginx 的模块
    - name: EXTRA_CONFIG
      label: 额外 configure 参数
      type: string
      placeholder: --with-http_v2_module
      desc: 原样拼接到 ./configure 末尾
`,
  readOpts: `# 方式一：直接用注入的环境变量（env 已注入，最常用）
echo "已选模块: $MODULES"
# 安装目标通常落在 $APPS_DIR 下（各包可自行定义 INSTALL_PATH 等变量拼版本目录）
./configure --prefix="$APPS_DIR/nginx-$APP_VERSION" $EXTRA_CONFIG $MODULES || exit 1

# 方式二：source options.env（与脚本同目录）
source "$PKG_PATH/options.env"

# 方式三：读 options.json（适合 python / jq）
python3 - "$PKG_PATH/options.json" <<'PY'
import json, sys
opts = json.load(open(sys.argv[1]))
print(opts.get("MODULES", ""))
PY
`,
  nginxYaml: `# webserver/nginx/app.yaml（节选）
name: nginx
version: [1.24.0]
actions:
  build: 编译安装
scripts:
  install: build_linux_amd64.sh
options:
  build:
    - name: MODULES
      label: 编译模块
      type: multiselect
      choices:
        - ssl
        - gzip
        - stub_status
        - ipv6
      default: ssl
      required: true
      desc: 勾选需要编译进 nginx 的模块，多选以空格分隔
    - name: EXTRA_CONFIG
      label: 额外 configure 参数
      type: string
      desc: 原样追加到 ./configure 末尾
`,
  nginxUse: `# build_linux_amd64.sh（节选）
# $MODULES / $EXTRA_CONFIG 已由系统注入 env（来自 options）
# prefix 落在 $APPS_DIR 下：仓库样例 INSTALL_PATH="$APPS_DIR/nginx-$APP_VERSION"
./configure \\
--prefix="$INSTALL_PATH" \\
--with-http_ssl_module \\
--with-http_gzip_static_module \\
\${EXTRA_CONFIG:-} \${MODULES:-} \\
|| exit 1

make -j"$CPU_NUM" || exit 1
make install || exit 1
`,
}
</script>

<script lang="ts">
const toc = [
  { id: 'sec-package', label: '一、包结构' },
  { id: 'sec-appyaml', label: '二、app.yaml 字段' },
  { id: 'sec-lifecycle', label: '三、生命周期与升级' },
  { id: 'sec-model', label: '四、执行模型' },
  { id: 'sec-env', label: '五、环境变量' },
  { id: 'sec-options', label: '六、options 定义' },
  { id: 'sec-read', label: '七、脚本如何读取' },
  { id: 'sec-limits', label: '八、命名与长度限制' },
  { id: 'sec-example', label: '九、完整示例' },
  { id: 'sec-trouble', label: '十、失败排查' },
]
export default { name: 'DevAppScriptGuide' }
</script>

<style scoped>
.app-script-guide .el-card {
  border: none;
}
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.header-left {
  display: flex;
  align-items: center;
}
.title {
  font-size: 15px;
  font-weight: 600;
}
.guide-body {
  display: flex;
  gap: 24px;
  align-items: flex-start;
}
.guide-toc {
  position: sticky;
  top: 76px;
  flex: 0 0 200px;
  border-right: 1px solid var(--el-border-color-lighter);
  padding: 4px 16px 16px 0;
}
.toc-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  margin-bottom: 8px;
}
.toc-item {
  display: block;
  font-size: 13px;
  color: var(--el-text-color-regular);
  line-height: 2;
  text-decoration: none;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.toc-item:hover {
  color: var(--el-color-primary);
}
.guide-content {
  flex: 1;
  min-width: 0;
  max-width: 900px;
}
.lead {
  color: var(--el-text-color-secondary);
}
.guide-content h2 {
  font-size: 17px;
  margin: 28px 0 12px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
.guide-content p,
.guide-content li {
  font-size: 13.5px;
  line-height: 1.9;
  color: var(--el-text-color-regular);
}
.guide-content code {
  font-family: 'JetBrains Mono', Consolas, Monaco, monospace;
  font-size: 12.5px;
  background: var(--el-fill-color-light);
  color: var(--el-color-primary);
  padding: 1px 5px;
  border-radius: 4px;
}
pre.code {
  background: var(--el-fill-color-light);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  padding: 12px 14px;
  font-family: 'JetBrains Mono', Consolas, Monaco, monospace;
  font-size: 12.5px;
  line-height: 1.75;
  overflow-x: auto;
  color: var(--el-text-color-primary);
  white-space: pre;
}
.sec-sub {
  margin: 16px 0 10px;
  font-size: 14px;
  color: var(--el-text-color-primary);
}
.doc-tip {
  margin: 12px 0;
}
.doc-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
  margin: 12px 0;
}
.doc-table th,
.doc-table td {
  border: 1px solid var(--el-border-color-lighter);
  padding: 8px 10px;
  text-align: left;
  vertical-align: top;
  line-height: 1.7;
}
.doc-table th {
  background: var(--el-fill-color-light);
  font-weight: 600;
  white-space: nowrap;
}
.footnote {
  margin-top: 32px;
  font-size: 12px;
  color: var(--el-text-color-placeholder);
}
@media (max-width: 1100px) {
  .guide-toc {
    display: none;
  }
}
</style>
