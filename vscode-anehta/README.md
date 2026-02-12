# AnehtaLanguage VSCode Extension

Official Visual Studio Code extension for **AnehtaLanguage** - a high-performance language with first-class vector/matrix support and WASM SIMD acceleration.

## Features

### 🎨 Syntax Highlighting

Complete syntax highlighting for all AnehtaLanguage features:

- **Primitives**: `int`, `float`, `string`, `bool`
- **Vectors & Matrices**: `vec`, `mat` with SIMD-optimized operations
- **Closures**: `|x, y| => expr` anonymous functions
- **Tables**: `{ key: value }` hash maps with compile-time GC
- **Control Flow**: `if`/`else`/`elseif`, `for`, `return`, `break`, `continue`
- **Special Blocks**: `timer { ... }` for auto-timing code execution

### ⚡ Vector/Matrix Operations (v0.2.0+)

Full support for first-class vector and matrix types with WASM SIMD acceleration:

```javascript
// Vectors
var v1 = [1.0, 2.0, 3.0]
var v2 = [4.0, 5.0, 6.0]

// Vector arithmetic (SIMD-accelerated)
var sum = v1 + v2        // Element-wise addition
var dot = v1 @ v2        // Dot product
var cross = v1 # v2      // Cross product (3D only)

// Swizzle (extract components)
var xy = v1.xy           // Get first 2 elements
var x = v1.x             // Get first element
var bgr = color.bgr      // Reorder components

// Matrices
var m = [1.0, 0.0; 0.0, 1.0]  // 2x2 identity matrix
var result = m * v1            // Matrix-vector multiply
var m2 = m * m                 // Matrix multiply
```

### 📋 Code Snippets

Type prefix and press `Tab` to expand:

#### Vectors
- `vec2` → 2D vector
- `vec3` → 3D vector
- `vec4` → 4D vector
- `vdot` → Dot product
- `vcross` → Cross product
- `vadd` → Vector addition
- `vscale` → Scalar multiplication

#### Matrices
- `mat2` → 2x2 identity matrix
- `mat3` → 3x3 identity matrix
- `mat4` → 4x4 identity matrix
- `mmul` → Matrix multiplication
- `mvmul` → Matrix-vector multiplication

#### Swizzle
- `sxy` → Extract `.xy` components
- `sxyz` → Extract `.xyz` components
- `srgba` → Extract `.rgba` color channels

#### Control Flow
- `func` → Function declaration
- `for` → For loop with counter
- `if` / `ife` / `ifel` → If/else statements
- `timer` → Timer block
- `closure` / `closureb` → Closures

#### Data Structures
- `var` / `vart` → Variable declaration
- `table` / `tablem` / `tablen` → Table literals

### 🔧 Operators

- **Arithmetic**: `+`, `-`, `*`, `/`, `%`, `^` (power)
- **Vector/Matrix**: `@` (dot product), `#` (cross product)
- **Random**: `~` (range operator, e.g., `1 ~ 100`)
- **Comparison**: `==`, `!=`, `<`, `>`, `<=`, `>=`
- **Logical**: `&&`, `||`, `!`
- **Assignment**: `=`, `+=`, `-=`, `*=`, `/=`, `%=`
- **Increment**: `++`, `--`

### 🛠️ Built-in Commands

- **Anehta: Build** - Compile `.ah` file to WASM
- **Anehta: Run** - Compile and execute with wasmtime
- **Anehta: Build & Run** - One-click build and run (appears in editor toolbar)

### ⚙️ Configuration

Configure the path to `anehta-cli.exe` in VSCode settings:

```json
{
  "anehta.cliPath": "E:\\RustProject\\AnehtaLanguage\\target\\release\\anehta-cli.exe"
}
```

## Requirements

- [AnehtaLanguage compiler](https://github.com/yourusername/AnehtaLanguage) installed
- Visual Studio Code 1.85.0 or higher

## Installation

### From VSIX
1. Download the `.vsix` file from releases
2. Open VSCode
3. Press `Ctrl+Shift+P` and run `Extensions: Install from VSIX...`
4. Select the downloaded `.vsix` file

### From Source
```bash
cd vscode-anehta
npm install
npm run compile
npx @vscode/vsce package
code --install-extension anehta-language-0.2.0.vsix
```

## Language Examples

### Vector Math with SIMD
```javascript
// Physics simulation with SIMD acceleration
var position = [0.0, 10.0, 0.0]
var velocity = [5.0, 0.0, 2.0]
var gravity = [0.0, -9.8, 0.0]
var dt = 0.016  // 60 FPS

// Update physics (all operations use WASM SIMD)
velocity = velocity + gravity * dt
position = position + velocity * dt

print("Position: ")
print(position)
print("Speed: ")
print(len(velocity))  // Vector length
```

### Matrix Transformations
```javascript
// 2D rotation matrix
var angle = 45.0
var rad = angle * 3.14159 / 180.0
var cos_a = 0.707  // cos(45°)
var sin_a = 0.707  // sin(45°)

var rotation = [cos_a, -sin_a;
                sin_a,  cos_a]

var point = [1.0, 0.0]
var rotated = rotation * point
print("Rotated point: ")
print(rotated)
```

### Closures and Higher-Order Functions
```javascript
func map(arr, f) {
    for (var i = 0; i < len(arr); i = i + 1) {
        arr[i] = f(arr[i])
    }
    return arr
}

var nums = [1.0, 2.0, 3.0, 4.0]
var squared = map(nums, |x| => x * x)
print(squared)  // [1.0, 4.0, 9.0, 16.0]
```

## Performance

All vector and matrix operations use **WASM SIMD** instructions:
- ✅ 128-bit vector operations (2 × f64 per instruction)
- ✅ Zero host function call overhead
- ✅ Works in both CLI (wasmtime) and browser
- ✅ Automatic tail handling for odd-length vectors
- ✅ Fully inlined - no runtime dispatch

## Release Notes

See [CHANGELOG.md](CHANGELOG.md) for detailed release history.

### 0.2.0 (Latest)
- Added complete Vec/Mat SIMD syntax support
- New operators: `@` (dot), `#` (cross)
- Enhanced snippets for vector/matrix operations
- Swizzle syntax highlighting

### 0.1.0
- Initial release with basic syntax highlighting
- Closures, tables, control flow
- Build/run commands

## Contributing

Issues and PRs welcome at [GitHub repository](https://github.com/yourusername/AnehtaLanguage)

## License

MIT License - see LICENSE file for details

---

**Enjoy coding with AnehtaLanguage! 🚀**
