# AnehtaLanguage

AnehtaLanguage 是一门编译到 WebAssembly 的实验性编程语言，由 Rust 实现，使用 wasmtime 作为运行时。语言设计初稿始于 2015.09.19。

## 特色

- **WASM SIMD 向量/矩阵** — 一等公民的向量和矩阵类型，完全内联的 SIMD 加速运算
- **原生随机运算符 `~`** — `1~6` 直接生成 1 到 6 的随机整数，和 `+` `-` 一样是一等运算符
- **多返回值** — 函数可以返回多个值，配合多变量赋值使用
- **闭包** — 支持捕获外部变量的 lambda 表达式
- **表 (Table)** — 内置字典类型，支持 `.field` 和 `["key"]` 两种访问方式
- **编译时表 GC** — 基于所有权分析的表内存管理，零运行时开销
- **计时器块 `timer {}`** — 内置代码性能测量语法
- **编译到 WASM** — 生成标准 WebAssembly 字节码

## 快速开始

### 构建编译器

```bash
cargo build --release
```

### 运行程序

```bash
# 编译并运行
anehta-cli run examples/demo.ah

# 仅编译为 .wasm
anehta-cli build examples/demo.ah
```

### Hello World

```javascript
var name = "Anehta"
print(name)

func add(a: int, b: int) -> int {
    return a + b
}
print(add(10, 20))
```

## 语言概览

### 变量与类型

```javascript
var x = 42                    // 整数
var name = "Anehta"           // 字符串
var alive = true              // 布尔值
var hp: int                   // 类型声明
var a, b = swap(1, 2)         // 多变量赋值
```

所有值在底层都是 i64。字符串使用 packed i64 编码（ptr<<32 | len）。

### 运算符

| 运算符 | 说明 | 示例 |
|--------|------|------|
| `+` `-` `*` `/` `%` | 基本算术 | `1 + 2 * 3` |
| `^` | 乘方 | `2 ^ 10` → 1024 |
| `~` | 随机数（闭区间） | `1 ~ 6` → 1 到 6 随机 |
| `++` `--` | 自增/自减 | `i++` |
| `>` `<` `>=` `<=` `==` `!=` | 比较 | `x > 10` |
| `&&` `\|\|` | 逻辑与/或 | `x > 0 && x < 100` |
| `+` | 字符串拼接 | `"Hello" + " World"` |
| `@` | 向量点积 | `v1 @ v2` → 标量 |
| `#` | 向量叉积 (3D) | `v1 # v2` → 向量 |
| `'` | 矩阵转置 | `m'` → 转置矩阵 |

### 函数

```javascript
func factorial(n: int) -> int {
    if (n <= 1) {
        return 1
    }
    return n * factorial(n - 1)
}
print(factorial(5))  // 120

// 多返回值
func swap(a: int, b: int) -> int, int {
    return b, a
}
var x, y = swap(1, 2)
```

### 控制流

```javascript
if (hp > 80) {
    print(1)
} elseif (hp > 50) {
    print(2)
} else {
    print(0)
}

for (var i = 0; i < 100; i = i + 1) {
    if (i % 2 == 0) { continue }
    if (i > 50) { break }
}

for (;;) {   // 无限循环
    break
}
```

### 闭包

```javascript
var double = |x| => x * 2
print(double(21))  // 42

var add = |x, y| => x + y
print(add(3, 4))   // 7

// 捕获外部变量
var base = 100
var addBase = |x| => x + base
print(addBase(42))  // 142

// 闭包体为块
var compute = |x| => {
    var result = x * x + 1
    return result
}
```

### 表 (Table)

```javascript
var player = { name: "Bob", hp: 200, mp: 50 }

// 点访问
print(player.hp)       // 200

// 括号访问
print(player["name"])  // Bob

// 字段赋值
player.hp = 150
player["mp"] = 25

// 嵌套表
var game = {
    player: { name: "Alice", hp: 100 },
    enemy: { name: "Slime", hp: 30 }
}
print(game.player.name)  // Alice

// 表字段存储闭包并调用
var readB = || => 10
var a = {getVal: readB}
print(a.getVal())     // 10

var math = {op: |x, y| => x + y}
print(math.op(3, 4))  // 7
```

### 向量与矩阵 (WASM SIMD)

AnehtaLanguage 将向量和矩阵作为**一等公民**，所有运算使用 **WASM SIMD** 内联实现，性能接近原生代码。

#### 向量运算

```javascript
// 向量字面量
var v1 = [1.0, 2.0, 3.0]
var v2 = [4.0, 5.0, 6.0]

// 逐元素运算 (SIMD 加速)
print(v1 + v2)       // [5.0, 7.0, 9.0]
print(v1 - v2)       // [-3.0, -3.0, -3.0]
print(v1 * v2)       // [4.0, 10.0, 18.0] (逐元素乘)

// 标量运算
print(v1 * 2.0)      // [2.0, 4.0, 6.0]
print(v1 + 10.0)     // [11.0, 12.0, 13.0]
print(v1 / 2.0)      // [0.5, 1.0, 1.5]

// 点积 (SIMD)
var dot = v1 @ v2    // 32.0 (1*4 + 2*5 + 3*6)

// 叉积 (3D向量)
var a = [1.0, 0.0, 0.0]
var b = [0.0, 1.0, 0.0]
var c = a # b        // [0.0, 0.0, 1.0] (z轴)

// 索引访问
print(v1[0])         // 1.0
v1[1] = 99.0

// Swizzle (分量提取)
print(v1.x)          // 1.0
print(v1.xy)         // [1.0, 2.0]
print(v1.xyz)        // [1.0, 2.0, 3.0]

// 向量长度
print(len(v1))       // 3
```

#### 矩阵运算

```javascript
// 矩阵字面量 (行主序，用 ; 分隔行)
var m1 = [1.0, 2.0; 3.0, 4.0]        // 2×2 矩阵
var m2 = [1.0, 2.0, 3.0; 4.0, 5.0, 6.0]  // 2×3 矩阵

// 逐元素运算 (SIMD 加速)
var m3 = [5.0, 6.0; 7.0, 8.0]
print(m1 + m3)       // [[6.0, 8.0], [10.0, 12.0]]
print(m1 - m3)       // [[-4.0, -4.0], [-4.0, -4.0]]
print(m1 * 2.0)      // [[2.0, 4.0], [6.0, 8.0]]

// 矩阵乘法 (SIMD 优化)
var result = m1 * m3              // 真正的矩阵乘法
print(result)                     // [[19.0, 22.0], [43.0, 50.0]]

// 矩阵×向量 (SIMD)
var v = [1.0, 0.0]
var transformed = m1 * v          // [1.0, 3.0]

// 转置 (循环展开优化)
var mt = m1'                      // 2×2 → 2×2 转置
print(mt)                         // [[1.0, 3.0], [2.0, 4.0]]

// 链式操作
var complex = (m1 + m3)'          // 先加法后转置
print((m1')')                     // 双重转置 = 原矩阵

// 索引访问
print(m1[0][1])      // 2.0
m1[1][0] = 99.0
```

#### 性能优势

所有向量/矩阵运算都是 **完全内联的 WASM SIMD 指令**，无函数调用开销：

- ✅ 使用 128-bit SIMD 向量（一次处理 2 个 f64）
- ✅ 零 host function 调用开销
- ✅ 浏览器和 CLI 通用
- ✅ 自动处理奇数长度（tail handling）

**性能对比**：
- 向量运算：~2x SIMD 加速
- 矩阵乘法：~2-4x SIMD 加速
- 转置：~1.3-1.5x 循环展开优化

### 计时器

```javascript
timer {
    var sum = 0
    for (var i = 0; i < 1000000; i = i + 1) {
        sum = sum + i
    }
    print(sum)
}
// 自动输出: ⏱ Timer: 12ms
```

### 随机运算符

```javascript
// 掷骰子
var dice = 1 ~ 6

// RPG 伤害计算
var baseDmg = 10
var damage = baseDmg + (1 ~ 20)

// 蒙特卡洛估算 (在表达式中使用)
var x = 0 ~ 1000
var y = 0 ~ 1000
```

## 项目结构

```
AnehtaLanguage/
├── crates/
│   ├── anehta-lexer/          # 词法分析器
│   ├── anehta-parser/         # 递归下降解析器
│   ├── anehta-codegen-wasm/   # WASM 代码生成
│   └── anehta-cli/            # 命令行工具 + wasmtime 运行时
├── examples/                   # 示例程序
│   ├── demo.ah                # 基础示例
│   ├── stress_test.ah         # 综合测试
│   ├── timer_demo.ah          # 性能基准测试
│   ├── table_test.ah          # 表功能测试
│   ├── table_gc_test.ah       # 表 GC 测试
│   ├── closure_table_test.ah  # 闭包+表测试
│   ├── simd_showcase.ah       # SIMD 向量/矩阵完整展示
│   ├── vec_simd_complete.ah   # 向量 SIMD 完整测试
│   ├── mat_simd_test.ah       # 矩阵 SIMD 测试
│   ├── mat_multiply_test.ah   # 矩阵乘法测试
│   └── transpose_test.ah      # 转置运算测试
├── vscode-anehta/             # VSCode 语法高亮扩展
├── LANGUAGE_SPEC.md           # 语言规范 (English)
└── AnehtaLanguage语法规范.md   # 语言规范 (中文)
```

## 编译流水线

```
.ah 源码 → Lexer (词法分析) → Parser (语法分析) → AST → WASM Codegen → .wasm → wasmtime 执行
```

## 运行时宿主函数

编译器生成的 WASM 模块通过以下宿主函数与运行时交互：

### 核心函数

| 函数 | 签名 | 说明 |
|------|------|------|
| `env.print` | `(i64)` | 打印整数 |
| `env.print_str` | `(i64)` | 打印字符串 |
| `env.print_float` | `(i64)` | 打印浮点数 |
| `env.input` | `() → i64` | 读取用户输入 |
| `env.random` | `(i64, i64) → i64` | 生成范围内随机数 |
| `env.clock` | `() → i64` | 获取时钟 (毫秒) |
| `env.str_concat` | `(i64, i64) → i64` | 字符串拼接 |

### 表操作

| 函数 | 签名 | 说明 |
|------|------|------|
| `env.table_new` | `() → i64` | 创建新表 |
| `env.table_get` | `(i64, i64) → i64` | 读取表字段 |
| `env.table_set` | `(i64, i64, i64)` | 设置表字段 |
| `env.table_free` | `(i64)` | 释放表 |

### 向量/矩阵输出

| 函数 | 签名 | 说明 |
|------|------|------|
| `env.print_vec` | `(i64)` | 打印向量 |
| `env.print_mat` | `(i64)` | 打印矩阵 |

**注**：向量/矩阵的**所有运算**（加减乘除、点积、叉积、转置等）都是**完全内联的 WASM SIMD 指令**，无需调用 host function，零运行时开销。

## VSCode 扩展

`vscode-anehta/` 目录包含 VSCode 语法高亮扩展 (v0.2.0)，支持：

- `.ah` 文件完整语法高亮
  - 向量/矩阵字面量 `[1, 2, 3]`, `[a, b; c, d]`
  - 新运算符 `@` `#` `'`
  - Swizzle 语法 `.x` `.xyz` `.rgba`
- 代码片段（Snippets）
  - 向量：`vec2`, `vec3`, `vec4`, `vdot`, `vcross`
  - 矩阵：`mat2`, `mat3`, `mat4`, `mmul`, `mvmul`
  - Swizzle：`sxy`, `sxyz`, `srgba`
- 一键 Build / Run 按钮

安装扩展：
```bash
cd vscode-anehta
npm install
npm run compile
npx @vscode/vsce package
code --install-extension anehta-language-0.2.0.vsix
```

## 许可证

MIT License - Copyright (c) 2025 Anehta
