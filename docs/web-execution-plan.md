# ブラウザでRestrict Languageコードを実行する方法

## 現状
- コンパイラがWASMとして動作し、WATコードを生成
- 生成されたコードは表示のみで実行されない
- WASI依存のため、そのままではブラウザで実行不可

## 実行可能にする方法

### 方法1: JavaScript関数をインポート
```javascript
// app.jsに追加
async function runGeneratedWasm(watCode) {
    // WATをWASMバイナリに変換
    const wasmModule = await WebAssembly.instantiate(watCode, {
        env: {
            js_print: (ptr, len) => {
                // メモリから文字列を読み取って表示
                const memory = wasmInstance.exports.memory;
                const bytes = new Uint8Array(memory.buffer, ptr, len);
                const text = new TextDecoder().decode(bytes);
                console.log(text);
                updateOutput(text);
            }
        }
    });
    
    // main関数を実行
    wasmModule.exports._start();
}
```

### 方法2: WASI-Polyfillを使用
```javascript
import { WASI } from '@wasmer/wasi';

const wasi = new WASI({
    args: [],
    env: {},
    bindings: {
        ...WASI.defaultBindings,
        fs: undefined // ファイルシステムは不要
    }
});

// WASMモジュールをWASIで実行
const wasmModule = await WebAssembly.instantiate(wasmBinary, {
    wasi_snapshot_preview1: wasi.wasiImport
});

wasi.start(wasmModule);
```

### 方法3: 別のコード生成器を使用
`codegen_web.rs`を使用して、ブラウザ向けのコードを生成：
- WASI importをJavaScript importに置換
- fd_writeの代わりにconsole.logを使用

## 推奨アプローチ
方法1が最もシンプルで、Restrict Language専用の実行環境を提供できます。