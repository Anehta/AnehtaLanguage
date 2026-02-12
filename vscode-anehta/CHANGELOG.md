# Changelog

## [0.2.0] - 2026-02-12

### Added
- **Vector/Matrix SIMD Support**: Complete syntax highlighting for vec/mat literals and operations
- **New Operators**:
  - `@` (dot product) - highlighted as vector operator
  - `#` (cross product) - highlighted as vector operator
- **New Types**: `vec`, `mat`, `float`, `f64` now recognized in type annotations
- **Vec/Mat Literals**: Enhanced highlighting for `[x, y, z]` (vectors) and `[a, b; c, d]` (matrices)
- **Swizzle Syntax**: Special highlighting for `.x`, `.xy`, `.xyz`, `.xyzw`, `.rgba` etc.
- **Built-in Functions**: Added `int()`, `float()`, `len()` to builtin function highlighting
- **New Snippets**:
  - `vec2`, `vec3`, `vec4` - Create 2D/3D/4D vectors
  - `mat2`, `mat3`, `mat4` - Create identity matrices
  - `vdot`, `vcross` - Vector operations
  - `vadd`, `vscale` - Vector arithmetic
  - `sxy`, `sxyz`, `srgba` - Swizzle shortcuts
  - `mmul`, `mvmul` - Matrix operations
  - `tofloat`, `vlen` - Type conversion and length

### Changed
- Updated all type patterns to include new vector/matrix types
- Enhanced bracket syntax to distinguish vec/mat literals from array access
- Improved field access pattern to support swizzle operations

## [0.1.0] - Initial Release

### Added
- Syntax highlighting for AnehtaLanguage (.ah files)
- Support for basic types: `int`, `string`, `bool`
- Keywords: `func`, `var`, `if`, `else`, `elseif`, `for`, `return`, `break`, `continue`, `timer`
- Operators: `+`, `-`, `*`, `/`, `%`, `^`, `~`, `++`, `--`
- Built-in functions: `print()`, `input()`
- Closure syntax: `|x| => expr`
- Table literals: `{ key: value }`
- Build and run commands integrated into editor
- Basic code snippets for common patterns
