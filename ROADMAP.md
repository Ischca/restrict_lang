# Restrict Language Development Roadmap

**Last Updated**: 2025-12-27
**Status**: Active Development
**Target**: v1.0 Release

---

## 🎯 Core Vision

**Restrict Language is a functional programming language with strict scope management for WebAssembly.**

### Core Values

1. **Scope Strictness** - Resources and values have clearly defined, enforceable scopes
2. **Affine Types** - Variables can be used at most once, preventing accidental duplication
3. **Zero-cost Abstractions** - Compile-time safety with no runtime overhead
4. **WebAssembly Target** - Efficient, deterministic execution without GC

---

## 📊 Current Status (as of 2025-12-27)

### ✅ Implemented & Stable (70-95%)

| Feature | Status | Notes |
|---------|--------|-------|
| Lexer | ✅ 100% | Full token support, comments working |
| Parser | ✅ 95% | OSV syntax, patterns, all constructs |
| Basic Type System | ✅ 90% | Int, Float, String, Boolean, Unit, Records |
| Affine Types (Basic) | ⚠️ 80% | Working but needs refinement |
| OSV Syntax | ✅ 95% | Right-associative calls functional |
| Lambda Expressions | ✅ 85% | Closures working, some edge cases |
| Pattern Matching (Parsing) | ✅ 90% | Option, List, Record patterns |
| Arena Memory | ✅ 85% | Basic arena allocation |
| Context System | ✅ 75% | Callback-based resource management |

### 🚧 Partially Implemented (30-70%)

| Feature | Status | Notes |
|---------|--------|-------|
| Pattern Matching (Codegen) | ⚠️ 60% | Some patterns missing codegen |
| Affine Types (Complex) | ⚠️ 70% | Multiple refs, complex expressions |
| Type Inference | ⚠️ 70% | Bidirectional working, needs expansion |
| WebAssembly Codegen | ⚠️ 60% | Basic constructs work, advanced incomplete |
| Module System | ⚠️ 40% | Structure exists, not fully functional |
| Standard Library | ⚠️ 50% | Core functions exist, incomplete |

### 🔬 Experimental / On Hold (0-30%)

| Feature | Status | Decision |
|---------|--------|----------|
| **Temporal Affine Types (TAT)** | ⚠️ 50% (parsing/AST) | **→ Experimental feature, v2.0 target** |
| Async/Await | 📋 Design only | Post v1.0 |
| Recursive Functions | ⚠️ 30% | Needs work |
| Higher-order Functions | ⚠️ 40% | map/filter/fold incomplete |

---

## 🎬 Development Strategy: TAT Postponement

### Decision: Move TAT to Experimental

**Rationale**:
1. **Scope strictness is already achieved** through:
   - Affine types (usage count strictness)
   - Context + Callback (scope boundary strictness)
   - Arena (memory scope strictness)
   - with blocks (explicit scope strictness)

2. **TAT adds complexity** without being essential to core value
3. **Implementation cost is very high** (~6-12 months for full TAT)
4. **Interactions with other features are undefined**

### TAT Status

- ✅ **Keep**: Syntax reserved (`~`, `within`, `lifetime`)
- ✅ **Keep**: Documentation archived for future reference
- ⚠️ **Move to experimental**: Implementation behind feature flag
- 📋 **Postpone**: Full implementation to v2.0+

---

## 🚀 Roadmap to v1.0

### Phase 1: Core Stabilization (1-2 months)

**Goal**: Make all core features production-ready

#### 1.1 Affine Types Completion ✅

**Status**: COMPLETED (2025-12-27)

**Completed Tasks**:
- [x] Fix ignored test: `test_function_params_affine`
- [x] Make semicolons optional after val bindings
- [x] Fix Unit type and () literal parsing
- [x] Implement affine checking for complex expressions
- [x] Add detailed error messages with suggestions
- [x] Test coverage: nested blocks, conditionals, mutable variables
  - 9 comprehensive affine tests (up from 4)
  - Coverage: basic violations, field access, nested blocks, conditionals, mutable vars

**Achievements**:
- ✅ All 46 tests passing
- ✅ No ignored tests
- ✅ Improved error messages with fix suggestions
- ✅ Semicolons now optional (better UX)

**Error Message Improvement**:
```
Before: Variable p has already been used (affine type violation)

After:  Affine type violation: variable 'p' has already been used.

        Affine types can only be used once. To fix this:
        - Use 'mut val' if you need to use the value multiple times
        - Use '.clone' to create a copy before the first use
        - Restructure your code to only use the value once
```

**Commits**:
- `2a8ccff` - fix: Parse Unit return types and unit literals ()
- `db45aaa` - feat: Make semicolons optional after val bindings
- `0c1979d` - test: Add comprehensive affine type tests
- `881b6e4` - feat: Improve affine type violation error messages

---

#### 1.2 Pattern Matching Code Generation

**Current Issues**:
- Parser handles patterns correctly
- Type checker validates patterns
- Codegen incomplete for Some/None/List patterns

**Tasks**:
- [ ] Implement Option pattern codegen (`Some(x)`, `None`)
- [ ] Implement List pattern codegen (`[]`, `[head | tail]`)
- [ ] Implement Record pattern codegen (`Record { x y }`)
- [ ] Add exhaustiveness checking in codegen
- [ ] Test all pattern combinations

**Success Criteria**: All pattern matching tests passing

---

#### 1.3 Context Standard Library

**Current Issues**:
- Context mechanism exists but underutilized
- No standard contexts for common resources
- Best practices not documented

**Tasks**:
- [ ] Implement `FileSystem` context
  ```rust
  context FileSystem {
      open: (String, (File) -> R) -> R
      read: File -> String
      write: (File, String) -> Unit
  }
  ```
- [ ] Implement `Database` context
  ```rust
  context Database {
      connect: (String, (Connection) -> R) -> R
      transaction: (Connection, (Transaction) -> R) -> R
  }
  ```
- [ ] Implement `HttpClient` context
- [ ] Document context pattern as best practice for resource management
- [ ] Create comprehensive examples

**Success Criteria**: 3+ standard contexts with full documentation

---

#### 1.4 Arena Enhancement

**Tasks**:
- [ ] Support nested arenas
- [ ] Add arena size tracking and overflow detection
- [ ] Implement arena growth strategies
- [ ] Document arena usage patterns
- [ ] Performance benchmarks

**Success Criteria**: Nested arenas working, documented patterns

---

### Phase 2: Language Completeness (2-3 months)

#### 2.1 Standard Library Expansion

**Tasks**:
- [ ] List operations: map, filter, fold, zip
- [ ] String operations: split, join, substring
- [ ] Option utilities: map, flatMap, unwrap_or
- [ ] Math functions: min, max, abs, etc.
- [ ] I/O functions integrated with contexts

**Success Criteria**: Usable standard library for real applications

---

#### 2.2 Module System Completion

**Tasks**:
- [ ] Import/export functionality
- [ ] Module path resolution
- [ ] Namespace management
- [ ] Circular dependency detection
- [ ] Module-level documentation

**Success Criteria**: Multi-file projects work correctly

---

#### 2.3 Error Handling

**Tasks**:
- [ ] Result type implementation
- [ ] Error propagation patterns
- [ ] Error context and messages
- [ ] Panic handling in WASM
- [ ] Graceful error recovery

**Success Criteria**: Robust error handling in all components

---

#### 2.4 Type System Polish

**Tasks**:
- [ ] Generic type inference improvements
- [ ] Type aliases
- [ ] Trait-like bounds (if needed)
- [ ] Better type error messages
- [ ] Type system documentation

**Success Criteria**: Type inference "just works" in most cases

---

### Phase 3: Production Ready (1-2 months)

#### 3.1 Tooling

**Tasks**:
- [ ] LSP server stability improvements
- [ ] VSCode extension polish
- [ ] Warder package manager completion
- [ ] Build system optimization
- [ ] Debugger integration (if feasible)

**Success Criteria**: Good developer experience

---

#### 3.2 Documentation

**Tasks**:
- [ ] Complete language tutorial
- [ ] API reference documentation
- [ ] Best practices guide
- [ ] Migration guide (if applicable)
- [ ] Example applications (3-5 real-world examples)

**Success Criteria**: New users can learn the language without assistance

---

#### 3.3 Testing & Validation

**Tasks**:
- [ ] Comprehensive test suite (>90% coverage)
- [ ] Integration tests for all features
- [ ] Performance benchmarks
- [ ] Stress testing (large files, deep nesting)
- [ ] Real-world application testing

**Success Criteria**: No known critical bugs

---

#### 3.4 Release Preparation

**Tasks**:
- [ ] Version 1.0 feature freeze
- [ ] Release notes preparation
- [ ] Website and landing page
- [ ] Package distribution (crates.io, etc.)
- [ ] Community setup (Discord, forums, etc.)

**Success Criteria**: Ready to announce v1.0

---

## 🔬 Post v1.0: Future Directions

### v1.1 - v1.x: Stability & Adoption

- Bug fixes and stability improvements
- Performance optimization
- Community feedback incorporation
- Additional standard library functions
- More example applications

### v2.0: Advanced Features (6+ months after v1.0)

**Temporal Affine Types Revival**:
- [ ] Formal specification completion
- [ ] Interaction with all v1.0 features defined
- [ ] Comprehensive test suite
- [ ] Gradual rollout as experimental → stable

**Other Potential Features**:
- [ ] Async/await (if TAT is stable)
- [ ] Effect system
- [ ] Advanced generics
- [ ] SIMD operations
- [ ] WebGPU backend

---

## 📋 Immediate Action Items (This Week)

### 1. Move TAT to Experimental

```bash
# Create experimental directory structure
mkdir -p src/experimental
mkdir -p docs/experimental

# Move TAT implementation
git mv src/lifetime_inference.rs src/experimental/

# Add feature flag to Cargo.toml
[features]
default = []
experimental-tat = []

# Update conditional compilation
#[cfg(feature = "experimental-tat")]
mod experimental;
```

**Files to update**:
- [ ] `Cargo.toml` - Add feature flag
- [ ] `src/lib.rs` - Conditional TAT modules
- [ ] `src/type_checker.rs` - Feature-gate TAT code
- [ ] `README.md` - Update feature status
- [ ] `docs/TAT_IMPLEMENTATION_STATUS.md` - Mark as experimental

---

### 2. Update Documentation

Create/update these files:
- [x] This file: `ROADMAP.md`
- [ ] `README.md` - Update implementation status section
- [ ] `docs/DEVELOPMENT_PLAN.md` - Detailed technical plan
- [ ] `CONTRIBUTING.md` - Guide for contributors on priorities

---

### 3. Fix Ignored Tests (After parser fix)

Priority order:
1. [ ] Fix parser to handle function definitions
2. [ ] `type_checker::tests::test_function_params_affine`
3. [ ] Any other ignored/skipped tests
4. [ ] Document why tests were ignored

---

## 🚨 Parser Issues Discovered and Fixed (2025-12-27)

### Issues Found

**Initial Symptom**:
- Test `test_function_params_affine` was ignored with TODO note
- When un-ignored, test passed with `Ok(())` instead of expected affine violation error

**Investigation Results** - Three distinct issues identified:

#### Issue 1: Incorrect Syntax Used ❌
```
# WRONG - Not EBNF v-1.0 compliant:
fun use_twice = p: Point { val a = p.x; val b = p.x; a }

# CORRECT - EBNF v-1.0 syntax:
fun use_twice: (p: Point) -> Unit = { val a = p.x; val b = p.x; () }
```
- Parser expects: `fun name: (params) -> ReturnType = { body }`
- See `RESTRICT_LANG_EBNF.md` line 211-214

#### Issue 2: Missing Semicolons Required ⚠️
```
# Parser fails to parse statement boundaries without semicolons:
val a = p.x
val b = p.x   // OSV parser consumes this incorrectly

# FIX: Add semicolons after val bindings:
val a = p.x;
val b = p.x;
```
- OSV parser is greedy and crosses statement boundaries
- Semicolons explicitly mark statement endings

#### Issue 3: Parser Bugs Fixed ✅
1. **Unit Type Parsing**: `parse_type` didn't handle `Unit` keyword
   - Added `type_name()` helper to accept both identifiers and `Unit`
   - Enables `-> Unit` return types

2. **Unit Literal Parsing**: `()` literal wasn't recognized as expression
   - `()` is lexed as `LParen` + `RParen` (two tokens), not `Token::Unit`
   - Added special case in `atom_expr` before general parenthesized expressions

### Fixes Applied

**Parser Changes** (`src/parser.rs`):
- [x] Added `type_name()` function to handle Unit keyword in types
- [x] Added `()` literal parsing in `atom_expr`
- [x] Added test `test_fun_decl_unit_return`

**Test Changes** (`src/type_checker.rs`):
- [x] Fixed `test_function_params_affine` with correct syntax
- [x] Added semicolons after val bindings
- [x] Removed `#[ignore]` attribute

### Result

✅ **test_function_params_affine now passes**
✅ **Affine type checking correctly detects parameter violations**

```
Type error: Variable p has already been used (affine type violation)
```

**Commit**: `2a8ccff` - fix: Parse Unit return types and unit literals ()

---

## 🎯 Success Metrics

### v1.0 Release Criteria

**Functionality**:
- ✅ All core features implemented and tested
- ✅ No ignored/skipped tests
- ✅ Pattern matching fully working
- ✅ Standard library usable
- ✅ Module system functional

**Quality**:
- ✅ >90% test coverage
- ✅ No known critical bugs
- ✅ Performance benchmarks acceptable
- ✅ Memory safety verified

**Documentation**:
- ✅ Complete tutorial
- ✅ API reference
- ✅ 5+ example applications
- ✅ Best practices guide

**Tooling**:
- ✅ LSP server stable
- ✅ VSCode extension working
- ✅ Warder package manager functional

---

## 👥 Resource Allocation

### Current Focus (80% of effort)

1. **Affine types completion** (30%)
2. **Pattern matching codegen** (25%)
3. **Context standard library** (15%)
4. **Documentation** (10%)

### Maintenance (20% of effort)

- Bug fixes
- Code reviews
- Issue triage

---

## 📞 Communication

### Status Updates

- Weekly progress summary (if active development)
- Monthly milestone reviews
- Public roadmap on GitHub

### Community

- GitHub Issues for bug reports
- Discussions for feature requests
- Discord/forum for community support (post v1.0)

---

## 🔄 Roadmap Review

This roadmap will be reviewed and updated:
- **Monthly**: Progress check and priority adjustment
- **Quarterly**: Major milestone assessment
- **After v1.0**: Planning for v2.0

---

## 📝 Appendix: TAT Decision Record

### Why TAT is Experimental

**Date**: 2025-12-27

**Decision**: Move Temporal Affine Types to experimental feature, targeting v2.0

**Context**:
- Core value is "scope strictness", already achieved through:
  - Affine types
  - Context + Callback
  - Arena memory
  - with blocks
- TAT implementation would take 6-12 months
- Interactions with other features undefined
- Risk of delaying v1.0 significantly

**Consequences**:
- ✅ Faster path to v1.0
- ✅ More stable core language
- ✅ TAT can be added later without breaking changes
- ⚠️ Some advanced use cases postponed
- ⚠️ Current TAT code will be feature-gated

**Alternatives Considered**:
1. Complete TAT now - Rejected due to time/complexity
2. Remove TAT entirely - Rejected, valuable for v2.0
3. Simplify TAT - Would still take significant time

**Review Date**: After v1.0 release (6+ months)

---

**End of Roadmap**

*This is a living document. Last updated: 2025-12-27*
