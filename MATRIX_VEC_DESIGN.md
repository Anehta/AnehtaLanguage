# AnehtaLanguage 矩阵与向量设计文档

> 状态：设计阶段 | 日期：2026-02-12

---

## 1. 设计原则

矩阵 (`mat`) 和向量 (`vec`) 是 AnehtaLanguage 的**一等公民**，与 `int`、`float`、`string`、`bool` 地位完全相同：

- 有原生字面量语法
- 有编译期类型跟踪 (`AhType::Vec`, `AhType::Mat`)
- 数据存在 WASM 线性内存（不是 host-side 不透明 handle）
- 运算符直接作用于 vec/mat
- `print` 自动派发
- 可作为函数参数、返回值、闭包捕获、table 字段

---

## 2. 值表示（打包 i64）

所有值统一为 i64，vec/mat 的数据区是连续 f64 数组，存在 WASM 线性内存 heap 上。

```
类型     i64 布局                         数据区
─────────────────────────────────────────────────────────
int      直接 i64 值                      无
float    f64 bits as i64                  无
string   ptr:32 | len:32                  连续 UTF-8 字节
vec      ptr:32 | len:32                  连续 f64 (每个 8 字节)
mat      ptr:32 | rows:16 | cols:16       连续 f64 (row-major, 每个 8 字节)
```

### 内存布局示意

```
vec [1.0, 2.0, 3.0]:

    i64 = (ptr << 32) | 3
    ptr ──▶ ┌──────────┬──────────┬──────────┐
            │ 1.0 (f64)│ 2.0 (f64)│ 3.0 (f64)│   24 bytes
            └──────────┴──────────┴──────────┘

mat [1, 2; 3, 4] (2×2, row-major):

    i64 = (ptr << 32) | (2 << 16) | 2
    ptr ──▶ ┌──────────┬──────────┬──────────┬──────────┐
            │ 1.0 (f64)│ 2.0 (f64)│ 3.0 (f64)│ 4.0 (f64)│   32 bytes
            └──────────┴──────────┴──────────┴──────────┘
              row 0                  row 1
```

- Vec 元素上限：2³² (约 40 亿)
- Mat 维度上限：65535 × 65535
- 元素访问：`ptr + (row * cols + col) * 8`

---

## 3. 类型系统

### 3.1 AhType 新增变体

```rust
enum AhType {
    Int,
    Float,
    Str,
    Vec,        // 新增
    Mat,        // 新增
    Closure(u32),
    Table(u32),
}
```

### 3.2 类型名映射

```rust
fn type_name_to_ah(name: &str) -> AhType {
    match name {
        "str" | "string" => AhType::Str,
        "float" | "f64"  => AhType::Float,
        "vec"             => AhType::Vec,
        "mat"             => AhType::Mat,
        _                 => AhType::Int,
    }
}
```

### 3.3 编译期推断规则

```
字面量:
    [1, 2, 3]                    → Vec
    [1, 2; 3, 4]                 → Mat
    [1, 2, 3
     4, 5, 6]                   → Mat

工厂:
    vec.zeros(n)                 → Vec
    vec.ones(n)                  → Vec
    mat.eye(n)                   → Mat
    mat.zeros(m, n)              → Mat
    mat.rand(m, n)               → Mat

运算结果:
    vec + vec                    → Vec
    vec * scalar                 → Vec
    mat + mat                    → Mat
    mat * mat                    → Mat
    mat * vec                    → Vec
    vec * mat                    → Vec
    vec @ vec                    → Float  (点积)
    vec # vec                    → Vec    (叉积)
    mat \ vec                    → Vec    (解方程)
    mat'                         → Mat    (转置)
    mat.det                      → Float
    mat.rank                     → Int
    mat.eigen                    → (Vec, Mat)  多返回值
    mat.svd                      → (Mat, Vec, Mat)
    mat.lu                       → (Mat, Mat, Mat)
    mat.qr                       → (Mat, Mat)
    mat[i, ..]                   → Vec    (行提取)
    mat[.., j]                   → Vec    (列提取)
```

### 3.4 类型标注

```javascript
var v -> vec
var m -> mat

func normalize(var v -> vec) -> vec { ... }
func decompose(var m -> mat) -> vec, mat { ... }
```

---

## 4. 字面量语法

### 4.1 判定规则

```
[表达式, ...]          无分号无换行     → vec
[表达式, ...; ...]     有分号           → mat
[表达式, ...           有换行           → mat
 表达式, ...]
```

### 4.2 向量

```javascript
var v = [1, 2, 3]
var v2 = [1.5, 2.7, 3.9]
var v3 = [x, y, z]           // 变量引用
var v4 = [1 + 2, 3 * 4, 5]   // 表达式
```

### 4.3 矩阵

```javascript
// 多行写法（换行分行）
var m = [1, 2, 3
         4, 5, 6
         7, 8, 9]

// 单行写法（分号分行）
var m2 = [1, 0; 0, 1]

// 强制 1×n 矩阵（尾部分号）
var row_mat = [1, 2, 3;]

// 3×1 矩阵
var col_mat = [1; 2; 3]
```

### 4.4 推导式

```javascript
// 向量推导（单迭代器 → vec）
var squares = [i ^ 2 | i <- 1..6]              // [1, 4, 9, 16, 25]
var evens = [i | i <- 1..20, i % 2 == 0]       // 带条件过滤

// 矩阵推导（双迭代器 → mat）
var H = [1.0 / float(i + j + 1) | i <- 0..3, j <- 0..3]   // 3×3 Hilbert 矩阵
var id = [if (i == j) { 1.0 } else { 0.0 } | i <- 0..4, j <- 0..4]  // 4×4 单位矩阵
```

---

## 5. 工厂构造器

### 5.1 向量

```javascript
var v1 = vec.zeros(5)                // [0, 0, 0, 0, 0]
var v2 = vec.ones(3)                 // [1, 1, 1]
var v3 = vec.fill(4, 7.0)            // [7, 7, 7, 7]
var v4 = vec.rand(5)                 // 5 个随机 f64 (0.0~1.0)
var v5 = vec.linspace(0, 1, 5)       // [0, 0.25, 0.5, 0.75, 1.0]
var v6 = vec.range(0, 10)            // [0, 1, 2, ..., 9]
var v7 = vec.range(0, 10, 2)         // [0, 2, 4, 6, 8]  带步长
```

### 5.2 矩阵

```javascript
var m1 = mat.eye(3)                  // 3×3 单位矩阵
var m2 = mat.zeros(3, 4)             // 3×4 全零
var m3 = mat.ones(2, 3)              // 2×3 全一
var m4 = mat.diag([1, 2, 3])         // 对角矩阵
var m5 = mat.rand(3, 3)              // 3×3 随机 (0.0~1.0)
var m6 = mat.randi(3, 3, 1, 10)      // 3×3 随机整数 (1~10)
var m7 = mat.fill(3, 3, 7.0)         // 3×3 全部填充 7.0
var m8 = mat.from(3, 3, |i, j| => float(i * 3 + j))  // 闭包构造
```

### 5.3 特殊矩阵构造（图形/3D）

```javascript
var T = mat.translate(1, 2, 3)       // 4×4 平移矩阵
var R = mat.rotate(axis, angle)      // 4×4 旋转矩阵（轴角）
var S = mat.scale(sx, sy, sz)        // 4×4 缩放矩阵
var Rx = mat.rotate_x(angle)         // 绕 X 轴
var Ry = mat.rotate_y(angle)         // 绕 Y 轴
var Rz = mat.rotate_z(angle)         // 绕 Z 轴
var M = mat.trs(t, r, s)             // 从 T, R, S 组装 4×4 变换矩阵
```

### 5.4 随机矩阵

```javascript
var Q = mat.rand_orthogonal(3)       // 随机正交矩阵
var P = mat.rand_positive_def(3)     // 随机正定矩阵
var S = mat.rand_sparse(100, 100, 0.1)  // 随机稀疏矩阵 (10% 非零)
```

### 5.5 分块矩阵构造

```javascript
var A = [1, 2; 3, 4]
var B = [5; 6]
var C = [7, 8, 9]

// 用矩阵拼矩阵，直接嵌套
var M = [A, B
         C   ]
// 等价于:
// [1, 2, 5
//  3, 4, 6
//  7, 8, 9]

// 对角分块
var D = mat.blkdiag(A, B, C)
```

---

## 6. 运算符

### 6.1 运算符总表

| 运算符 | 含义 | 左操作数 | 右操作数 | 结果 |
|--------|------|----------|----------|------|
| `+` | 加法 | vec/mat | vec/mat/scalar | vec/mat |
| `-` | 减法 | vec/mat | vec/mat/scalar | vec/mat |
| `*` | 矩阵乘法 / 逐元素乘(vec) / 标量乘 | mat/vec/scalar | mat/vec/scalar | mat/vec |
| `/` | 标量除 | vec/mat | scalar | vec/mat |
| `\` | 左除 (解方程 Ax=b) | mat | vec/mat | vec/mat |
| `^` | 矩阵幂 | mat | int | mat |
| `.^` | 逐元素幂 | vec/mat | vec/mat/scalar | vec/mat |
| `@` | 点积 | vec | vec | float |
| `#` | 叉积 (仅 3D) | vec | vec | vec |
| `'` | 转置 (后缀) | mat | — | mat |
| `-` | 取负 (前缀) | vec/mat | — | vec/mat |

### 6.2 优先级

```
优先级          运算符                         结合性
─────────────────────────────────────────────────────
1 (最低)       +   -                          左
2              *   /   \   %   ~              左
3              ^   .^                         左
4              @   #                          左
5              '                              后缀
6 (最高)       ()  []  .field                 —
```

### 6.3 标量自动广播

```javascript
var m = [1, 2; 3, 4]

m + 1         // 每个元素 +1 → [2, 3; 4, 5]
m * 3         // 每个元素 ×3 → [3, 6; 9, 12]
3 * m         // 同上（交换律）
m / 2.0       // 每个元素 ÷2
m ^ 2         // 矩阵幂 M*M（不是逐元素）
m .^ 2        // 逐元素平方 → [1, 4; 9, 16]
```

### 6.4 `*` 语义规则

```
mat * mat     →  矩阵乘法
mat * vec     →  矩阵向量乘（vec 当列向量，结果为 vec）
vec * mat     →  向量矩阵乘（vec 当行向量，结果为 vec）
vec * vec     →  逐元素乘（Hadamard）
mat * scalar  →  标量乘（每个元素）
scalar * mat  →  同上（交换律）
vec * scalar  →  标量乘
scalar * vec  →  同上
```

### 6.5 vec ↔ mat 转换

```javascript
var v = [1, 2, 3]
var col = v.to_col         // vec → n×1 mat
var row = v.to_row         // vec → 1×n mat
```

### 6.6 广播规则

```javascript
var m = [1, 2, 3; 4, 5, 6]    // 2×3
var v = [10, 20, 30]           // vec (3)

var r = m + v                  // 每一行都加 v → [11,22,33; 14,25,36]
var c = m + [100; 200]         // 每一列都加   → [101,102,103; 204,205,206]
var s = m + 1                  // 标量广播     → [2,3,4; 5,6,7]

// 用 :r / :c 明确指定广播轴（避免歧义）
m + v:r                        // 按行广播
m + col:c                      // 按列广播
```

广播规则（同 NumPy）：
1. scalar 与任何形状 → 扩展为相同形状
2. vec(n) 与 mat(m×n) → 沿行广播
3. mat(m×1) 与 mat(m×n) → 列向量沿列广播
4. mat(1×n) 与 mat(m×n) → 行向量沿行广播
5. 不兼容形状 → 运行时报错

---

## 7. 索引与切片

### 7.1 向量

```javascript
var v = [10, 20, 30, 40, 50]

v[0]              // 10（第一个）
v[-1]             // 50（末尾）
v[-2]             // 40（倒数第二）
v[1..4]           // [20, 30, 40]（切片）
v[0] = 99         // 赋值
```

### 7.2 矩阵

```javascript
var m = [1, 2, 3; 4, 5, 6; 7, 8, 9]

// 单元素
m[0, 0]            // 1
m[1, 2]            // 6
m[0, 0] = 99       // 赋值

// 行/列提取 → vec
m[0, ..]           // [1, 2, 3]  第 0 行
m[.., 1]           // [2, 5, 8]  第 1 列

// 切片 → mat
m[0..2, 0..2]      // [1,2; 4,5]  左上 2×2 子矩阵
m[1..3, ..]        // [4,5,6; 7,8,9]  第 1-2 行

// 步长切片
m[0..3..2, ..]     // 每隔 1 行 → 第 0 行和第 2 行

// 负索引
m[-1, ..]          // [7, 8, 9]  最后一行
```

### 7.3 花式索引（用索引向量取）

```javascript
var v = [10, 20, 30, 40, 50]
var idx = [0, 3, 4]

var picked = v[idx]            // [10, 40, 50]
v[idx] = [99, 88, 77]         // scatter 写入 → [99, 20, 30, 88, 77]

// 矩阵行选取
var rows = M[[0, 2, 4], ..]   // 取第 0、2、4 行
```

### 7.4 布尔掩码索引

```javascript
var v = [3, -1, 4, -1, 5, 9]

// 比较 → 布尔向量
var mask = v > 0               // [true, false, true, false, true, true]

// 掩码索引 → 过滤
var pos = v[v > 0]             // [3, 4, 5, 9]

// 掩码赋值
v[v < 0] = 0                  // [3, 0, 4, 0, 5, 9]

// 矩阵也行
var M = [1, -2; -3, 4]
M[M < 0] = 0                  // [1, 0; 0, 4]  一行实现 ReLU

// 组合条件
var selected = v[v > 2 && v < 8]   // [3, 4, 5]
```

### 7.5 where 三元选择

```javascript
// where(条件, 真值, 假值) — 逐元素
var result = vec.where(v > 0, v, 0)    // [3, 0, 4, 0, 5]
var M2 = mat.where(A > B, A, B)        // 逐元素取较大值
```

---

## 8. Swizzle（向量分量访问）

### 8.1 读取

```javascript
var v = [1, 2, 3]

// 单分量 → float
v.x              // 1
v.y              // 2
v.z              // 3

// 多分量 → vec（任意组合、任意顺序、可重复）
v.xy             // [1, 2]
v.zx             // [3, 1]
v.zy             // [3, 2]
v.xyz            // [1, 2, 3]
v.zyx            // [3, 2, 1]
v.xx             // [1, 1]
v.xxx            // [1, 1, 1]
v.xyzz           // [1, 2, 3, 3]
```

### 8.2 写入

```javascript
var v = [1, 2, 3]

v.x = 10          // → [10, 2, 3]
v.xy = [10, 20]   // → [10, 20, 3]
v.zx = [30, 10]   // → [10, 2, 30]

// 写入不能重复分量
v.xx = [1, 2]     // 编译错误：swizzle 赋值不能有重复分量
```

### 8.3 两套命名（互为别名）

| 索引 | 位置 | 颜色 |
|------|------|------|
| 0 | `x` | `r` |
| 1 | `y` | `g` |
| 2 | `z` | `b` |
| 3 | `w` | `a` |

```javascript
var color = [0.5, 0.8, 1.0, 1.0]

color.r              // 0.5
color.rgb            // [0.5, 0.8, 1.0]
color.bgr            // [1.0, 0.8, 0.5]
color.a              // 1.0

// 两套不能混用
color.xg             // 编译错误：不能混用 xyzw 和 rgba
```

### 8.4 编译实现

完全编译期处理，零运行时开销：

```
v.x      →  解包 ptr, f64.load(ptr + 0*8)              → float
v.z      →  解包 ptr, f64.load(ptr + 2*8)              → float
v.xy     →  分配新 vec(2), 复制 [0],[1]                  → vec
v.zyx    →  分配新 vec(3), 复制 [2],[1],[0]              → vec
v.x = 10 →  解包 ptr, f64.store(ptr + 0*8, 10.0)       → void
v.xy = u →  解包两边 ptr, 复制 u[0]→v[0], u[1]→v[1]    → void
```

编译器看到 `.` 后面跟的是纯 `xyzw` 或 `rgba` 字母组合时，走 swizzle 路径而不是 field access 路径。长度超过 4 或包含非法字母时报编译错误。

### 8.5 Swizzle 实际用例

```javascript
// 3D 图形常见操作
var pos = [1, 2, 3, 1]        // 齐次坐标
var pos3d = pos.xyz            // 取前三个分量
var flat = pos.xz              // 取 xz 平面投影

// 颜色操作
var pixel = [0.2, 0.5, 0.8, 1.0]
var opaque = pixel.rgb         // 去掉 alpha
var gray = [pixel.r, pixel.r, pixel.r]

// swizzle + 运算（手动叉积展开）
var crossed = v.yzx * u.zxy - v.zxy * u.yzx

// TRS 配合
var t, r, s = M.trs
var pos2d = t.xy               // 只取平移的 xy
```

---

## 9. 解构赋值

```javascript
// 向量解构
var [x, y, z] = [10, 20, 30]
// x=10, y=20, z=30

// 矩阵解构
var [a, b; c, d] = [1, 2; 3, 4]
// a=1, b=2, c=3, d=4

// 和函数返回配合
var [u1, u2; u3, u4], s, [v1, v2; v3, v4] = A.svd

// 用 _ 丢弃不需要的
var [_, b; _, d] = M           // 只要第二列
var vals, _ = M.eigen          // 只要特征值
```

---

## 10. 属性与方法

### 10.1 形状信息

| 属性 | 适用 | 返回类型 | 说明 |
|------|------|----------|------|
| `.len` | vec/mat | int | 元素总数 |
| `.rows` | mat | int | 行数 |
| `.cols` | mat | int | 列数 |
| `.shape` | mat | (int, int) | (行, 列) |

### 10.2 标量属性

| 属性 | 适用 | 返回类型 | 说明 |
|------|------|----------|------|
| `.det` | mat (方阵) | float | 行列式 |
| `.trace` | mat (方阵) | float | 迹 (对角线之和) |
| `.rank` | mat | int | 秩 |
| `.cond` | mat | float | 条件数 (2-范数) |
| `.cond1` | mat | float | 条件数 (1-范数) |

### 10.3 范数

| 属性 | 适用 | 说明 |
|------|------|------|
| `.norm` | vec/mat | L2 / Frobenius 范数 |
| `.norm1` | vec/mat | L1 / 1-范数 |
| `.normi` | vec/mat | L∞ / ∞-范数 |
| `.norms` | mat | 谱范数 (最大奇异值) |

### 10.4 变换

| 属性/方法 | 适用 | 返回类型 | 说明 |
|-----------|------|----------|------|
| `'` | mat | mat | 转置 |
| `.inv` | mat (方阵) | mat | 逆矩阵 |
| `.pinv` | mat | mat | Moore-Penrose 伪逆 |
| `.adj` | mat (方阵) | mat | 伴随矩阵 |
| `.unit` | vec | vec | 归一化 (v / ‖v‖) |
| `.to_col` | vec | mat | 转为 n×1 矩阵 |
| `.to_row` | vec | mat | 转为 1×n 矩阵 |

### 10.5 矩阵分解（完整）

| 方法 | 返回值 | 说明 |
|------|--------|------|
| `.lu` | (mat, mat, mat) | PA = LU → (L, U, P) |
| `.qr` | (mat, mat) | A = QR → (Q, R) |
| `.svd` | (mat, vec, mat) | A = UΣVᵀ → (U, S, Vt) |
| `.eigen` | (vec, mat) | A = VΛV⁻¹ → (eigenvalues, eigenvectors) |
| `.eigenvals` | vec | 仅特征值 |
| `.singvals` | vec | 仅奇异值 |
| `.cholesky` | mat | A = LLᵀ → L (正定矩阵) |
| `.schur` | (mat, mat) | A = QTQᵀ → (Q, T) |
| `.hessenberg` | (mat, mat) | A = QHQᵀ → (Q, H) |
| `.ldl` | (mat, vec, mat) | A = LDLᵀ → (L, D对角, Lᵀ)，对称矩阵 |
| `.polar` | (mat, mat) | A = UP → (U 正交, P 半正定) |
| `.jordan` | (mat, mat) | A = PJP⁻¹ → (P, J Jordan标准形) |
| `.tridiag` | (mat, mat) | 对称→三对角 → (Q, T) |
| `.bidiag` | (mat, mat, mat) | 双对角化 → (U, B, V) |
| `.trs` | (vec, mat, vec) | 4×4变换矩阵 → (平移vec3, 旋转mat3, 缩放vec3) |
| `.cr` | (mat, mat) | CR 分解 (低秩近似) → (C, R) |
| `.nmf` | (mat, mat) | 非负矩阵分解 → (W, H)，其中 A ≈ W*H |
| `.geigen(B)` | (vec, mat) | 广义特征分解 Av = λBv → (eigenvalues, eigenvectors) |

### 10.6 元素聚合

| 属性 | 适用 | 返回类型 | 说明 |
|------|------|----------|------|
| `.max` | vec/mat | float | 最大元素 |
| `.min` | vec/mat | float | 最小元素 |
| `.sum` | vec/mat | float | 元素之和 |
| `.mean` | vec/mat | float | 均值 |
| `.diag` | mat | vec | 对角线 → 向量 |
| `.argmax` | vec | int | 最大值索引 |
| `.argmin` | vec | int | 最小值索引 |

### 10.7 按轴聚合

| 方法 | 适用 | 返回类型 | 说明 |
|------|------|----------|------|
| `.sum_rows` | mat | vec | 按行求和 → 列向量 |
| `.sum_cols` | mat | vec | 按列求和 → 行向量 |
| `.max_rows` | mat | vec | 按行最大 |
| `.min_cols` | mat | vec | 按列最小 |
| `.mean_rows` | mat | vec | 按行均值 |
| `.mean_cols` | mat | vec | 按列均值 |

### 10.8 逐元素数学

| 方法 | 适用 | 说明 |
|------|------|------|
| `.abs` | vec/mat | 绝对值 |
| `.sqrt` | vec/mat | 开方 |
| `.log` | vec/mat | 自然对数 |
| `.exp` | vec/mat | eˣ |
| `.sin` | vec/mat | 正弦 |
| `.cos` | vec/mat | 余弦 |
| `.tan` | vec/mat | 正切 |
| `.asin` | vec/mat | 反正弦 |
| `.acos` | vec/mat | 反余弦 |
| `.atan` | vec/mat | 反正切 |
| `.round` | vec/mat | 四舍五入 |
| `.floor` | vec/mat | 向下取整 |
| `.ceil` | vec/mat | 向上取整 |
| `.clamp(lo, hi)` | vec/mat | 钳制到 [lo, hi] |

### 10.9 向量专属

| 方法 | 返回类型 | 说明 |
|------|----------|------|
| `.dot(v)` | float | 点积（等价于 `@`） |
| `.cross(v)` | vec | 叉积（等价于 `#`，仅 3D） |
| `.proj(v)` | vec | 在 v 上的投影 |
| `.angle(v)` | float | 与 v 的夹角 (弧度) |
| `.outer(v)` | mat | 外积 → 矩阵 |
| `.kron(v)` | mat | Kronecker 积 |

### 10.10 形状变换

| 方法 | 返回类型 | 说明 |
|------|----------|------|
| `.reshape(m, n)` | mat | 重塑 (元素总数不变) |
| `.flatten` | vec | 展平为一维 |
| `.sort` | vec | 排序（返回新 vec） |
| `.argsort` | vec | 排序后的原始索引 |
| `.reverse` | vec | 反转 |
| `.shuffle` | vec | 随机打乱 |
| `.unique` | vec | 去重 |
| `.count(x)` | int | 值 x 出现次数 |
| `.bincount` | vec | 每个值出现次数 |

### 10.11 函数式操作

```javascript
var doubled  = v.map(|x| => x * 2)
var big      = v.filter(|x| => x > 3)
var total    = v.reduce(|a, b| => a + b)
```

### 10.12 累积 / 差分 / 滑动窗口

```javascript
var v = [1, 2, 3, 4, 5]

// 累积
v.cumsum              // [1, 3, 6, 10, 15]
v.cumprod             // [1, 2, 6, 24, 120]

// 差分
v.diff                // [1, 1, 1, 1]      一阶差分
v.diff(2)             // [0, 0, 0]          二阶差分

// 滑动窗口
v.rolling(3).mean     // [_, _, 2, 3, 4]    3-元素滑动平均
v.rolling(3).max      // [_, _, 3, 4, 5]
v.rolling(3).sum      // [_, _, 6, 9, 12]
```

### 10.13 卷积 / FFT

```javascript
// 卷积
var kernel = [1, 0, -1]
var edges = signal.conv(kernel)

// FFT
var spectrum = v.fft           // 快速傅里叶变换
var restored = spectrum.ifft    // 逆变换
```

### 10.14 统计

```javascript
var data = [2, 4, 4, 4, 5, 5, 7, 9]

data.mean                // 5.0
data.median              // 4.5
data.std                 // 标准差
data.var                 // 方差
data.percentile(25)      // 第 25 百分位
data.percentile(75)      // 第 75 百分位
data.histogram(5)        // 5 个 bin 的直方图

// 协方差矩阵
var cov = mat.cov(data_matrix)

// 相关系数矩阵
var corr = mat.corr(data_matrix)
```

### 10.15 布尔判定

| 属性 | 说明 |
|------|------|
| `.is_square` | 是否方阵 |
| `.is_symmetric` | 是否对称 |
| `.is_orthogonal` | 是否正交 |
| `.is_positive_def` | 是否正定 |
| `.is_invertible` | 是否可逆 |

### 10.16 高级矩阵运算

| 方法 | 返回类型 | 说明 |
|------|----------|------|
| `.expm` | mat | 矩阵指数 eᴬ |
| `.logm` | mat | 矩阵对数 ln(A) |
| `.sqrtm` | mat | 矩阵平方根 A^(1/2) |
| `.cofactor(i, j)` | float | 余子式 |
| `.minor(i, j)` | float | 子式 |
| `.ref` | mat | 行阶梯形 |
| `.rref` | mat | 简化行阶梯形 |
| `.nullspace` | mat | 零空间的基 |
| `.colspace` | mat | 列空间的基 |
| `.rowspace` | mat | 行空间的基 |
| `.poly` | vec | 特征多项式系数 |
| `.inner(m)` | float | Frobenius 内积 tr(AᵀB) |

### 10.17 拼接

```javascript
var h = mat.hstack(a, b)       // 水平拼接 [A | B]
var v = mat.vstack(a, b)       // 垂直拼接 [A; B]
```

---

## 11. 数值计算

### 11.1 数值微积分

```javascript
var x = vec.linspace(0, 3.14, 100)
var y = x.map(|v| => sin(v))

// 数值积分（梯形法）
var area = vec.trapz(x, y)            // ≈ 2.0

// 数值微分
var dy = vec.gradient(y, x)            // dy/dx ≈ cos(x)

// 数值 Jacobian
var J = mat.jacobian(|v| => [v.x^2 + v.y, v.x * v.y], [1.0, 2.0])
```

### 11.2 线性方程组

```javascript
var A = [2, 1; 5, 3]
var b = [4; 7]

// 左除：解 Ax = b
var x = A \ b

// 超定系统自动走最小二乘
var big_A = mat.rand(10, 3)
var big_b = mat.rand(10, 1)
var x_ls = big_A \ big_b       // m > n 时自动 lstsq

// Sylvester 方程 AX + XB = C
var X = mat.sylvester(A, B, C)

// Lyapunov 方程 AX + XAᵀ = Q
var X2 = mat.lyapunov(A, Q)
```

---

## 12. Einstein 求和

一行搞定任意张量缩并：

```javascript
// 矩阵乘法 C_ik = Σ_j A_ij * B_jk
var C = ein("ij,jk->ik", A, B)

// 迹 = Σ_i A_ii
var t = ein("ii->", A)

// 转置
var T = ein("ij->ji", A)

// 批量矩阵乘法
var C = ein("bij,bjk->bik", A_batch, B_batch)

// 外积
var O = ein("i,j->ij", u, v)

// 点积
var d = ein("i,i->", u, v)
```

---

## 13. 管道链式调用

```javascript
// 用 |> 把矩阵操作串起来
var result = A |> .inv |> * B |> .norm
// 等价于 (A.inv * B).norm

// 数据处理流水线
var clean = data
    |> .filter(|x| => x > 0)
    |> .map(|x| => x / x.max)
    |> .sort
```

---

## 14. ASCII 可视化

```javascript
var v = [1, 4, 2, 8, 5, 7, 3, 6]

v.plot
// 输出:
//  8 |    *
//  7 |         *
//  6 |              *
//  5 |      *
//  4 | *
//  3 |           *
//  2 |   *
//  1 |*
//    +---------------

// 散点图
mat.scatter(x, y)

// 矩阵热力图
M.heatmap
```

---

## 15. 稀疏矩阵

```javascript
// 构造
var s = mat.sparse(1000, 1000)
s[0, 0] = 1.0
s[999, 999] = 1.0

// 从三元组构造
var sp = mat.sparse_from([0,1,2], [0,1,2], [1.0,2.0,3.0], 3, 3)

// 稀疏单位矩阵
var si = mat.sparse_eye(1000)

// 运算符相同，自动分派
var result = sp * v

// 转换
var dense = sp.to_dense
var sparse = m.to_sparse
var nnz = sp.nnz               // 非零元素数量
```

---

## 16. 随机矩阵（配合 `~` 运算符）

```javascript
// Anehta 独有：~ 运算符和矩阵字面量结合
var M = [1~10, 1~10; 1~10, 1~10]     // 每个元素都是 1~10 随机数

// 工厂
var Q = mat.rand_orthogonal(3)         // 随机正交矩阵
var P = mat.rand_positive_def(3)       // 随机正定矩阵
var S = mat.rand_sparse(100, 100, 0.1) // 10% 非零
```

---

## 17. 词法层变更

### 新增 Token

```rust
enum TokenType {
    // ... 已有 ...

    // 以下为新增
    At,            // @   点积
    Hash,          // #   叉积
    Backslash,     // \   左除
    Apostrophe,    // '   转置 (后缀)
    DotPow,        // .^  逐元素幂
    DotDot,        // ..  范围
    DotDotLt,      // ..< 范围 (不含右端)
    ArrowLeft,     // <-  推导式迭代
    Pipe,          // |>  管道
}
```

### `[]` 内部解析模式

进入 `[` 后启用矩阵解析模式：
- 逗号 = 列分隔
- 分号 = 行分隔
- 换行 = 行分隔（不再是语句分隔 `Newline` token）
- `|` = 推导式分隔符（非闭包）
- `<-` = 迭代器绑定
- `..` = 范围

---

## 18. 语法层变更

### 新增 AST 节点

```rust
/// 向量字面量: [expr, expr, ...]
pub struct VecLiteral {
    pub elements: Vec<Expr>,
    pub span: Span,
}

/// 矩阵字面量: [expr, expr; expr, expr]
pub struct MatLiteral {
    pub rows: Vec<Vec<Expr>>,
    pub span: Span,
}

/// 推导式: [expr | var <- range, ...]
pub struct Comprehension {
    pub body: Box<Expr>,
    pub iterators: Vec<CompIterator>,
    pub condition: Option<Box<BooleanExpr>>,
    pub span: Span,
}

pub struct CompIterator {
    pub var_name: String,
    pub range: Box<Expr>,
}

/// 范围: start..end 或 start..end..step
pub struct RangeExpr {
    pub start: Box<Expr>,
    pub end: Box<Expr>,
    pub step: Option<Box<Expr>>,
    pub inclusive: bool,
    pub span: Span,
}

/// 切片: m[spec, spec]
pub struct SliceAccess {
    pub object: Box<Expr>,
    pub row_slice: SliceSpec,
    pub col_slice: Option<SliceSpec>,  // None = 向量单维索引
    pub span: Span,
}

pub enum SliceSpec {
    Single(Box<Expr>),
    Range(RangeExpr),
    All,                               // ..
    Fancy(Vec<Expr>),                  // [0, 2, 4]
    Mask(Box<BooleanExpr>),            // v > 0
}

/// 解构赋值目标
pub enum DestructTarget {
    VecDestruct(Vec<String>, Span),          // [x, y, z]
    MatDestruct(Vec<Vec<String>>, Span),     // [a, b; c, d]
    Discard(Span),                           // _
    Name(String, Span),                      // 普通变量
}
```

### Expr 新增变体

```rust
pub enum Expr {
    // ... 已有 ...
    VecLiteral(VecLiteral),
    MatLiteral(MatLiteral),
    Comprehension(Comprehension),
    Range(RangeExpr),
    SliceAccess(SliceAccess),
    Transpose(Box<Expr>, Span),        // 后缀 '
    Pipeline(Box<Expr>, Box<Expr>, Span),  // |>
    EinsteinSum(String, Vec<Box<Expr>>, Span),  // ein("...", ...)
}
```

### BinaryOp 新增

```rust
pub enum BinaryOp {
    // ... 已有: Add, Sub, Mul, Div, Power, Mod, Rand ...
    DotPow,       // .^  逐元素幂
    At,           // @   点积
    Hash,         // #   叉积
    Backslash,    // \   左除
}
```

---

## 19. 编译层变更

### 19.1 三级编译策略

**Level 1: 内联 WASM 指令（零 host 开销）**

| 操作 | 编译方式 |
|------|----------|
| `[1, 2, 3]` 字面量 | 一系列 f64.const + f64.store |
| `v[i]` 索引读 | i64 解包 ptr + i*8, f64.load |
| `v[i] = x` 索引写 | f64.store |
| `m[i, j]` 索引读 | ptr + (i*cols+j)*8, f64.load |
| `v * scalar` | WASM 循环: load, f64.mul, store |
| `.rows` `.cols` `.len` | i64 位运算提取 |
| swizzle `v.xy` | 编译期偏移量, f64.load |

**Level 2: WASM 内部循环（中等复杂）**

| 操作 | 编译方式 |
|------|----------|
| `vec + vec` | 单层循环逐元素 |
| `mat + mat` | 单层循环逐元素 |
| `mat * vec` | 双层循环 |
| `mat * mat` | 三层循环 |
| `vec @ vec` | 累加循环 |
| `vec # vec` | 展开 6 次运算 |
| `.norm` | 累加平方 + f64.sqrt |
| 推导式 | 循环 + store |

**Level 3: Host 函数（复杂算法）**

| 操作 | Host 函数 |
|------|-----------|
| `.lu` | env.mat_lu(i64) → i64, i64, i64 |
| `.qr` | env.mat_qr(i64) → i64, i64 |
| `.svd` | env.mat_svd(i64) → i64, i64, i64 |
| `.eigen` | env.mat_eigen(i64) → i64, i64 |
| `.cholesky` | env.mat_cholesky(i64) → i64 |
| `.schur` | env.mat_schur(i64) → i64, i64 |
| `.ldl` | env.mat_ldl(i64) → i64, i64, i64 |
| `.polar` | env.mat_polar(i64) → i64, i64 |
| `.jordan` | env.mat_jordan(i64) → i64, i64 |
| `.tridiag` | env.mat_tridiag(i64) → i64, i64 |
| `.bidiag` | env.mat_bidiag(i64) → i64, i64, i64 |
| `.trs` | env.mat_trs(i64) → i64, i64, i64 |
| `.cr` | env.mat_cr(i64) → i64, i64 |
| `.nmf` | env.mat_nmf(i64) → i64, i64 |
| `.geigen` | env.mat_geigen(i64, i64) → i64, i64 |
| `.inv` | env.mat_inv(i64) → i64 |
| `.pinv` | env.mat_pinv(i64) → i64 |
| `.det` | env.mat_det(i64) → i64 |
| `.rank` | env.mat_rank(i64) → i64 |
| `.solve` (A\b) | env.mat_solve(i64, i64) → i64 |
| `.rref` | env.mat_rref(i64) → i64 |
| `.expm` | env.mat_expm(i64) → i64 |
| `.fft` | env.vec_fft(i64) → i64 |
| `.ifft` | env.vec_ifft(i64) → i64 |
| `.conv` | env.vec_conv(i64, i64) → i64 |
| `print_vec` | env.print_vec(i64) |
| `print_mat` | env.print_mat(i64) |

Host 函数通过 `caller.get_export("memory")` 直接读写 WASM 线性内存，与 `str_concat` 同模式。

### 19.2 内存分配

vec/mat 使用现有 bump allocator（`__heap_base` 起始）：

```
构造 [1.0, 2.0, 3.0]:
    1. heap_ptr 当前值 = dest
    2. f64.store(dest + 0, 1.0)
    3. f64.store(dest + 8, 2.0)
    4. f64.store(dest + 16, 3.0)
    5. heap_ptr += 24
    6. 返回 (dest << 32) | 3
```

### 19.3 print 派发

```rust
match expr_type {
    AhType::Int        => call env.print,
    AhType::Float      => call env.print_float,
    AhType::Str        => call env.print_str,
    AhType::Vec        => call env.print_vec,      // 新增
    AhType::Mat        => call env.print_mat,       // 新增
    AhType::Table(_)   => /* 现有逻辑 */,
    AhType::Closure(_) => /* 现有逻辑 */,
}
```

---

## 20. 内存管理

复用 table 的编译期所有权模型，零运行时开销。

### 20.1 赋值语义

```javascript
var v = [1, 2, 3]
var v2 = v             // 值复制（整块 f64 数组 memcpy）
v2[0] = 99             // 不影响 v
```

vec/mat 采用**值语义**，和 int/float 一致。赋值 = 深复制。

### 20.2 所有权跟踪

```rust
// FuncCtx 新增
pub owned_vecs: Vec<String>,    // 本函数分配的 vec 变量
pub owned_mats: Vec<String>,    // 本函数分配的 mat 变量
```

规则与 table 一致：
- 变量初始化为 -1 (sentinel)，`vec_free(-1)` 是 no-op
- 重新赋值前先 free 旧值
- 函数退出时 free 所有 owned_vecs/owned_mats
- `return v` 跳过 free（所有权转移给调用者）
- 参数不 free（borrowed）

---

## 21. 与现有特性的交互

### 21.1 存入 table

```javascript
var t = { position: [1.0, 2.0, 3.0], transform: mat.eye(4) }
```

table 的 field 类型推断出 Vec / Mat。

### 21.2 闭包捕获

```javascript
var scale = 2.0
var doubled = v.map(|x| => x * scale)

var A = [1, 2; 3, 4]
var transformed = mat.from(3, 3, |i, j| => float(A[i, j]) * 2.0)
```

### 21.3 函数参数与返回

```javascript
func normalize(var v -> vec) -> vec {
    return v / v.norm
}

func decompose(var m -> mat) -> vec, mat {
    var vals, vecs = m.eigen
    return vals, vecs
}
```

### 21.4 for 循环

```javascript
var v = [10, 20, 30]
for (var i = 0; i < v.len; i++) {
    print(v[i])
}
```

### 21.5 timer

```javascript
timer {
    var A = mat.rand(100, 100)
    var inv = A.inv
}
```

---

## 22. 完整示例

```javascript
// ═══════════════════════════════════════
//  AnehtaLanguage 矩阵全特性展示
// ═══════════════════════════════════════

// ── 向量 ──
var v = [1, 2, 3]
var u = [4, 5, 6]
print(v @ u)                           // 点积: 32
print(v # u)                           // 叉积: [-3, 6, -3]
print(v.norm)                          // 范数: 3.7416...
print(v.unit)                          // 单位向量

// ── swizzle ──
var pos = [1, 2, 3, 1]
print(pos.xyz)                         // [1, 2, 3]
print(pos.zyx)                         // [3, 2, 1]
pos.xy = [10, 20]

// ── 矩阵字面量 ──
var A = [4, 7
         2, 6]
var b = [1, 2]

// ── 解线性方程组 ──
var x = A \ b
print(x)

// ── 转置与矩阵乘法 ──
var ATA = A' * A
print(ATA)

// ── 特征分解 ──
var vals, vecs = A.eigen
print(vals)
print(vecs)

// ── SVD ──
var U, S, Vt = A.svd
var reconstructed = U * mat.diag(S) * Vt

// ── TRS ──
var M = mat.trs([1,2,3], mat.rotate_y(0.5), [1,1,1])
var t, r, s = M.trs
print(t.xy)

// ── 推导式 ──
var H = [1.0 / float(i + j + 1) | i <- 0..4, j <- 0..4]
print(H)
print(H.det)
print(H.cond)

// ── 解构赋值 ──
var [a, b2; c, d] = A
print(a)

// ── 布尔掩码 ──
var data = [3, -1, 4, -1, 5, 9]
var pos_data = data[data > 0]          // [3, 4, 5, 9]
data[data < 0] = 0                    // [3, 0, 4, 0, 5, 9]

// ── 最小二乘法 ──
var X = mat.rand(10, 3)
var y = mat.rand(10, 1)
var beta = (X' * X).inv * X' * y       // 正规方程
var beta2 = X \ y                       // 等价，更简洁

// ── Einstein 求和 ──
var C = ein("ij,jk->ik", A, A')        // A * Aᵀ

// ── 管道链式 ──
var result = A |> .inv |> * b |> .norm

// ── 累积与统计 ──
var values = [1, 2, 3, 4, 5]
print(values.cumsum)                   // [1, 3, 6, 10, 15]
print(values.mean)                     // 3.0
print(values.std)                      // 标准差

// ── 滑动窗口 ──
var smooth = values.rolling(3).mean

// ── FFT ──
var freq = values.fft
var back = freq.ifft

// ── 随机矩阵 + ~ ──
var dice_matrix = [1~6, 1~6; 1~6, 1~6]

// ── 闭包 + 矩阵 ──
var scale = |m, factor| => m * factor
print(scale(A, 2.0))

// ── ASCII 可视化 ──
var wave = [sin(float(i) / 5.0) | i <- 0..30]
wave.plot

// ── timer 测性能 ──
timer {
    var big = mat.rand(100, 100)
    var inv = big.inv
    var diff = (big * inv - mat.eye(100)).norm
    print(diff)
}
```

---

## 23. 一等公民检查清单

| 特性 | int | float | string | vec | mat |
|------|-----|-------|--------|-----|-----|
| 字面量 | `42` | `3.14` | `"hi"` | `[1,2,3]` | `[1,2;3,4]` |
| WASM 内存 | 直接 | 直接 | data 段 | heap | heap |
| i64 编码 | 值 | bits | ptr\|len | ptr\|len | ptr\|rows\|cols |
| AhType | Int | Float | Str | Vec | Mat |
| 运算符 | `+-*/` | `+-*/` | `+` | `+-*@#` | `+-*\'^` |
| print 派发 | print | print_float | print_str | print_vec | print_mat |
| 类型标注 | `->int` | `->float` | `->str` | `->vec` | `->mat` |
| 函数返回 | ok | ok | ok | ok | ok |
| 多返回值 | ok | ok | ok | ok | ok |
| 闭包捕获 | ok | ok | ok | ok | ok |
| table 字段 | ok | ok | ok | ok | ok |
| swizzle | — | — | — | ok | — |
| 推导式 | — | — | — | ok | ok |
| 解构赋值 | — | — | — | ok | ok |
| 布尔掩码 | — | — | — | ok | ok |
| 值语义 | ok | ok | ok(不可变) | ok(复制) | ok(复制) |
| 编译期 free | N/A | N/A | bump | 所有权 | 所有权 |

---

## 24. 实施阶段建议

```
Phase 1 — 基础 (vec 字面量 + 算术 + 索引)
    词法: [], .., @, #, \, ', .^ 等新 token
    语法: VecLiteral, SliceAccess
    类型: AhType::Vec
    编译: 字面量构造, 索引, swizzle, vec±vec, vec*scalar, @, #
    运行时: print_vec

Phase 2 — 矩阵基础 (mat 字面量 + 乘法 + 转置)
    语法: MatLiteral, Transpose
    类型: AhType::Mat
    编译: 字面量构造, 索引, mat*mat, mat*vec, 转置 '
    运行时: print_mat

Phase 3 — 运算符 + 索引完整化
    \(左除), .^(逐元素幂)
    广播, 切片, 花式索引, 布尔掩码
    解构赋值 [x, y, z] = ...

Phase 4 — 工厂 + 推导式
    mat.eye/zeros/ones/rand/diag/from
    vec.zeros/ones/linspace/range
    推导式语法 [expr | var <- range]
    分块矩阵构造

Phase 5 — 属性 + 基础分解
    .det .trace .rank .norm .inv .pinv
    .lu .qr .svd .eigen .cholesky
    host 函数实现 (nalgebra 或手写)

Phase 6 — 完整分解
    .schur .hessenberg .ldl .polar .jordan
    .tridiag .bidiag .trs .cr .nmf .geigen
    .rref .nullspace .colspace .rowspace

Phase 7 — 高级特性
    Einstein 求和 ein("...", ...)
    管道 |>
    滑动窗口 .rolling(), 累积 .cumsum/.cumprod
    统计 .std/.var/.median/.percentile
    FFT .fft/.ifft, 卷积 .conv()
    数值微积分 vec.trapz/vec.gradient/mat.jacobian
    mat.sylvester/mat.lyapunov
    ASCII 可视化 .plot/.heatmap
    稀疏矩阵

Phase 8 — 3D/图形
    mat.translate/rotate/scale/rotate_x/rotate_y/rotate_z
    mat.trs() 构造 + .trs 分解
    mat.rand_orthogonal/rand_positive_def/rand_sparse
```
