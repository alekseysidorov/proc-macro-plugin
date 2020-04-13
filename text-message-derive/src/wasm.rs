use anyhow::anyhow;
use proc_macro::TokenStream;
use std::{path::Path, str::FromStr};
use wasmtime::*;

struct WasmBuf<'a> {
    offset: usize,
    len: usize,
    instance: &'a Instance,
    memory: &'a Memory,
}

const WASM_PTR_LEN: usize = 4;

impl<'a> WasmBuf<'a> {
    pub fn from_host_buf(instance: &'a Instance, bytes: impl AsRef<[u8]>) -> Self {
        let bytes = bytes.as_ref();
        let len = bytes.len();

        let mut wasm_buf = Self::new(instance, len);
        wasm_buf.as_mut().copy_from_slice(bytes);
        wasm_buf
    }

    pub fn new(instance: &'a Instance, len: usize) -> Self {
        let offset = Self::toy_alloc(instance, len);
        let memory = Self::get_memory(instance);

        Self {
            offset: offset as usize,
            len,
            instance,
            memory,
        }
    }

    pub fn from_raw_ptr(instance: &'a Instance, offset: i32) -> Self {
        let offset = offset as usize;
        let memory = Self::get_memory(instance);

        let len = unsafe {
            let buf = memory.data_unchecked();

            let mut len_bytes = [0; WASM_PTR_LEN];
            len_bytes.copy_from_slice(&buf[offset..offset + WASM_PTR_LEN]);

            u32::from_le_bytes(len_bytes)
        };

        Self {
            offset,
            len: len as usize,
            memory,
            instance,
        }
    }

    pub fn as_ref(&self) -> &[u8] {
        unsafe {
            let begin = self.offset + WASM_PTR_LEN;
            let end = begin + self.len;

            &self.memory.data_unchecked()[begin..end]
        }
    }

    pub fn as_mut(&mut self) -> &mut [u8] {
        unsafe {
            let begin = self.offset + WASM_PTR_LEN;
            let end = begin + self.len;

            &mut self.memory.data_unchecked_mut()[begin..end]
        }
    }

    fn raw_parts(&self) -> (i32, i32) {
        let data_ptr = self.offset + WASM_PTR_LEN;
        (data_ptr as i32, self.len as i32)
    }

    fn get_memory(instance: &'a Instance) -> &'a Memory {
        instance.get_export("memory").unwrap().memory().unwrap()
    }

    fn toy_alloc(instance: &'a Instance, len: usize) -> usize {
        let toy_alloc = instance
            .get_export("toy_alloc")
            .expect("export named `toy_alloc` not found")
            .func()
            .expect("export `toy_alloc` was not a function")
            .get1::<i32, i32>()
            .unwrap();

        toy_alloc(len as i32).unwrap() as usize
    }

    fn toy_free(instance: &'a Instance, offset: usize) {
        let toy_free = instance
            .get_export("toy_free")
            .expect("export named `toy_free` not found")
            .func()
            .expect("export `toy_free` was not a function")
            .get1::<i32, ()>()
            .unwrap();

        toy_free(offset as i32).unwrap();
    }
}

impl Drop for WasmBuf<'_> {
    fn drop(&mut self) {
        Self::toy_free(self.instance, self.len);
    }
}

pub struct WasmMacro {
    module: Module,
}

impl WasmMacro {
    pub fn from_file(file: impl AsRef<Path>) -> anyhow::Result<Self> {
        // A `Store` is a sort of "global object" in a sense, but for now it suffices
        // to say that it's generally passed to most constructors.
        let store = Store::default();
        let module = Module::from_file(&store, file)?;
        // We start off by creating a `Module` which represents a compiled form
        // of our input wasm module. In this case it'll be JIT-compiled after
        // we parse the text format.
        Ok(Self { module })
    }

    pub fn proc_macro_derive(
        &self,
        fun: &str,
        item: TokenStream,
    ) -> anyhow::Result<TokenStream> {
        // To pass token stream between environments we have to serialize it to strings.
        let item = item.to_string();

        let instance = Instance::new(&self.module, &[])?;
        // Get a pointer to the desired wasm function.
        let proc_macro_attribute_fn = instance
            .get_export(fun)
            .ok_or_else(|| anyhow!("Unable to find `{}` method in the export table", fun))?
            .func()
            .ok_or_else(|| anyhow!("export {} is not a function", fun))?
            .get2::<i32, i32, i32,>()?;

        // To pass a string to wasm, we have allocate memory on the wasm side and copy the
        // data into it.
        let item_buf = WasmBuf::from_host_buf(&instance, item);
        // Serialize `WasmBuf` into (ptr, len) format to pass data into the wasm side.
        let (item_ptr, item_len) = item_buf.raw_parts();
        // Invoke desired function and get pointer to the resulting data.
        let ptr = proc_macro_attribute_fn(item_ptr, item_len).unwrap();
        // Fetch data to the host.
        let res = WasmBuf::from_raw_ptr(&instance, ptr);
        let res_str = std::str::from_utf8(res.as_ref())?;
        TokenStream::from_str(&res_str).map_err(|_| anyhow!("Unable to parse token stream"))
    }
}
