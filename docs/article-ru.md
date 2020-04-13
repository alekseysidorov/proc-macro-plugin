# Расширяем возможности процедурных макросов с помощью WASM

В рамках продолжения своих исследований различных аспектов процедурных макросов хочу поделиться
подходом к расширению их возможностей. Напомню, что процедурные макросы позволяют добавить в язык
элемент метапрограммирования и тем самым существенно упростить рутинные операции, такие как
сериализация или обработка запросов. По своей сути макросы являются плагинами к компилятору,
которые компилируются до сборки крейта, в котором они используются. У таких макросов есть некоторые
существенные недостатки.

- Сложность с поддержкой таких макросов в IDE. По сути дела нужно как-то научить анализатор кода
  самостоятельно компилировать, загружать и исполнять эти самые макросы с учетом всех особенностей.
  Это весьма нетривиальная задача.
- Так как макросы самодостаточные и ничего не знают друг о друге, то нет никакой возможности делать
  композицию макросов, что иногда могло быть полезным.

## Хабракат

По поводу решения первой проблемы сейчас ведутся [эксперименты][watt] с компиляцией всех
процедурных макросов в WASM модули, что позволит в будущем вообще отказаться от их компиляции
на целевой машине, а заодно и решить проблему с их поддержкой в IDE.

Что касается второй проблемы, то в этой статье я как раз собираюсь рассказать о своем подходе
к решению данной проблемы. По сути дела нам необходим такой макрос, который бы мог с помощью
атрибутов подгружать какие-то дополнительные макросы и объединять их в конвеер. В самом простейшем
случае можно просто представить нечто в роде такого:

Пусть у нас имеется некоторый макрос `TextMessage`, который выводит для заданного типа трейты
`ToString` и `FromStr` используя в качестве текстового представления некоторый кодек.
У разных типов сообщений может быть различный кодек, причем их полный список со временем
может расширятся, а у каждого кодека может быть свой уникальный набор атрибутов.

```rust
#[derive(Debug, Serialize, Deserialize, PartialEq, TextMessage)]
#[text_message(codec = "serde_json", params(pretty))]
struct FooMessage {
    name: String,
    description: String,
    value: u64,
}
```

Чтобы сделать такой макрос возможным, мы должны динамически подгружать реализации кодеков
в процессе выполнения макроса. Можно вынести кодеки в подключаемую библиотеку и просто загружать
их через [libloading], но это очень неудобно и еще больше отдалит нас от возможности поддержки
макросов в IDE. Вместо этого теоретически возможно написать такой вот кодек на динамическом языке
типа Питона, но тогда нам придется писать для Питона аналоги [`syn`] и [`quote`], что будет больше
напоминать Сизифов труд, чем реальное решение проблемы.
Наиболее же простым и удобным видится вариант скомпилировать кодек в WASM модуль, объединив плюсы
обоих подходов. Именно таким путем я предлагаю и пойти.

## Выбираем подход к реализации

На первый взгляд кажется, что проблема уже решена в рамках [watt] и можно просто использовать его
для загрузки и выполнения WASM модулей, но с этим подходом есть один весьма неприятный недостаток.
Для своей работы [watt] использует модифицированный крейт [`proc-macro2`], что частенько приводит к
непонятным или трудноуловимым проблемам. Например, у меня не компилировался `darling` или если я
забывал подменять `proc-macro2`, то получал в рантайме неочевидные ошибки.

В результае я решил, что лучше уж пользоваться ванильным [`proc-macro2`], а в качестве WASM
рантайма взять какой-нибудь из самых популярных. В результате, мой субьективный выбор пал на
[wasmtime], этот рантайм разрабатывается сообществом [bytecodealliance], в состав которого входят
такие гиганты, как Mozilla, Intel и RedHat. И хотя [wasmtime] сейчас выглядит еще достаточно сырым,
в нем не хватает документации, хороших примеров, но развивается он очень быстро и улучшается прямо
на глазах

## Взаимодействие хостом и таргетом

**Disclaimer**: этот раздел писался еще до того, появилась возможность генерировать
[интерфейс модуля] при помощи макросов. С другой стороны, в нем рассматривается самая
низкоуровневая работа с WASM модулями, что позволяет нам лучше понять принцип его работы.
Погнали!

В самом простом виде интерфейс плагина для нашего процедурного макроса должен представлять из себя
вариацию на тему:

```rust
pub fn implement_codec(input: TokenStream) -> TokenStream;
```

Но мы не можем передавать произвольные объекты между таргетом и хостом, нам необходимо
их сериализовать в универсальное представление, которое не будет зависить от особенностей хоста.
По счастью `TokenStream` можно преобразовывать в обычную строку и обратно, поэтому в реальности
мы будем использовать нечто в таком духе:

```rust
pub fn implement_codec(input: &str) -> String;
```

К великому сожалению, вот так просто взять и передать хостовую строчку, а уж тем более
передать строчку от таргета к хосту, не получится и на то есть серьезные причины:

В целях обеспечения безопасности ~~и большей стабильности, Республика будет реорганизована нами в
первую Галактическую Империю, во имя сохранности и во имя блага общества!~~ память WASM рантайма
отделена от хостовой, с точки зрения хоста это просто плоский массив байт, в котором находится
код программы, глобальные переменные, стек и куча. Есть возможность сделать так, чтобы память была
расширяемой, то если если при очередном выделении памяти нам не хватает места, то верхняя граница
памяти автоматически увеличивается. Индекс ячейки в этом самом массиве используется в качестве
указателя внутри таргета, но мы не можем просто взять и записать строчку в случайный участок
памяти и отдать таргету индекс его начала, потому что снаружи мы не знаем то, как таргет в
реальности использует память, где у него находится стек, а где куча.
Но мы можем пойти на хитрость: с хоста обратиться к менеджеру памяти таргета и попросить у него
аллоцировать нам участок памяти.

```rust
#[no_mangle]
pub unsafe extern "C" fn toy_alloc(size: i32) -> i32 {
    let size_bytes: [u8; 4] = size.to_le_bytes();
    let mut buf: Vec<u8> = Vec::with_capacity(size as usize + size_bytes.len());
    // Первые 4 байта - это длина общая куска памяти, она нам еще понадобится в дальнейшем.
    buf.extend(size_bytes.iter());
    to_host_ptr(buf)
}

unsafe fn to_host_ptr(mut buf: Vec<u8>) -> i32 {
    let ptr = buf.as_mut_ptr();
    // Просто забываем о выделеном участке памяти, позволяя ему "утечь", таким образом
    // мы передаем его во владение хосту.
    mem::forget(buf);
    ptr as *mut c_void as usize as i32
}

#[no_mangle]
pub unsafe extern "C" fn toy_free(ptr: i32) {
    let ptr = ptr as usize as *mut u8;
    let mut size_bytes = [0u8; 4];
    ptr.copy_to(size_bytes.as_mut_ptr(), 4);
    // Вычитываем общую длину куска памяти для того, чтобы корректно выполнить его очистку.
    let size = u32::from_le_bytes(size_bytes) as usize;
    // Собираем вектор, о котором мы ранее "забыли" в методе `to_host_ptr` и таким образом даем
    // его деструктору вызваться нормальным образом и очистить ранее выделенный участок памяти.
    Vec::from_raw_parts(ptr, size, size);
}
```

В принципе, ничего хитрого на самом деле в этом нет, примерно этим же занимается [`wasm_bindgen`].

Теперь попробуем создать свой первый WASM модуль для нашего процедурного макроса.
Для этого создадим крейт с единственной публичной функцией, она будет принимать указатель на начало строчки и длину строчки в байтах.

```rust
#[no_mangle]
pub unsafe extern "C" fn implement_codec(
    item_ptr: i32,
    item_len: i32,
) -> i32 {
    let item = str_from_raw_parts(item_ptr, item_len);
    let item = TokenStream::from_str(&item).expect("Unable to parse item");

    // Здесь уже вызывается типичная функция, реализующая процедурный макрос.
    // `fn(item: TokenStream) -> TokenStream`
    let tokens = codec::implement_codec(item);
    let out = tokens.to_string();
    
    to_host_buf(out)
}

pub unsafe fn str_from_raw_parts<'a>(ptr: i32, len: i32) -> &'a str {
    let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
    std::str::from_utf8(slice).unwrap()
}
```

Код хостовой части состоит из двух основных компонент, первым из которых является 
загрузчик WASM модуля.

```rust

pub struct WasmMacro {
    module: Module,
}

impl WasmMacro {
    // Конструктор нашего макроса расширения.
    pub fn from_file(file: impl AsRef<Path>) -> anyhow::Result<Self> {
        // Загружаем и компилируем WASM модуль, находящийся по заданному пути.
        let store = Store::default();
        let module = Module::from_file(&store, file)?;
        Ok(Self { module })
    }

    // Вызываем метод с именем `fun` внутри нашего модуля, в котором содержится основная логика
    // преобразования входного TokenStream в выходной.
    pub fn proc_macro_derive(
        &self,
        fun: &str,
        item: TokenStream,
    ) -> anyhow::Result<TokenStream> {
        // Как уже описывалось ранее, чтобы передавать TokenStream между средами, нам необходимо
        // преобразовать его в строку.
        let item = item.to_string();

        // Создаем конкретный экземпляр модуля, с которым и будем работать.
        let instance = Instance::new(&self.module, &[])?;
        // Получаем указатель на нужную нам функцию, в данном случае это описаная выше
        // `implement_codec`.
        let proc_macro_attribute_fn = instance
            .get_export(fun)
            .ok_or_else(|| anyhow!("Unable to find `{}` method in the export table", fun))?
            .func()
            .ok_or_else(|| anyhow!("export {} is not a function", fun))?
            .get2::<i32, i32, i32,>()?;

        // Для передачи данных строки внутрь WASM модуля используем специальную обертку,
        // о которой я подробнее расскажу ниже.
        let item_buf = WasmBuf::from_host_buf(&instance, item);
        // Получим из обертки указатель на начало строки и ее длину в байтах
        let (item_ptr, item_len) = item_buf.raw_parts();
        // А теперь вызываем искомый метод и в результате получаем указатель
        // на начало строки с выходным TokenStream. 
        let ptr = proc_macro_attribute_fn(item_ptr, item_len).unwrap();
        // Оборачиваем сырой указатель и читаем получившуюся строку.
        let res = WasmBuf::from_raw_ptr(&instance, ptr);
        let res_str = std::str::from_utf8(res.as_ref())?;
        // В заключительном этапе парсим строку в TokenStream и возращаем выше.
        TokenStream::from_str(&res_str).map_err(|_| anyhow!("Unable to parse token stream"))
    }
}
```

Теперь давайте чуть подробнее рассмотрим WasmBuf модуль: по сути дела это умный указатель,
который владеет некоторой частью памяти, выделенной при помощи `toy_alloc`. Рассмотрим
самые интересные его части, а остальной код можно посмотреть в репозитории.

```rust
struct WasmBuf<'a> {
    // Индекс начала выделенного буфера, проще говоря, указатель на его начало.
    offset: usize,
    // Длина буфера в байтах.
    len: usize,
    // Ссылка на инстанс модуля, в котором выделялась память
    instance: &'a Instance,
    // Ссылка на всю память, связанную с этим инстансом.
    memory: &'a Memory,
}

const WASM_PTR_LEN: usize = 4;

impl<'a> WasmBuf<'a> {
    // Самый простой конструктор буфера: мы просто при помощи `toy_alloc`
    // запрашиваем искомое число байт.
    pub fn new(instance: &'a Instance, len: usize) -> Self {
        let memory = Self::get_memory(instance);
        // Выделяем память и получаем на нее указатель.
        let offset = Self::toy_alloc(instance, len);

        Self {
            offset: offset as usize,
            len,
            instance,
            memory,
        }
    }

    // Намного удобнее не просто запрашивать буфер, а потом руками заполнять его, а сразу
    // передать ссылку на байты, которые мы хотим в него записать.
    pub fn from_host_buf(instance: &'a Instance, bytes: impl AsRef<[u8]>) -> Self {
        let bytes = bytes.as_ref();
        let len = bytes.len();

        let mut wasm_buf = Self::new(instance, len);
        // Копируем байты с хостового буфера в буфер таргета.
        wasm_buf.as_mut().copy_from_slice(bytes);
        wasm_buf
    }

    // Если же буфер был выделен внутри таргета, то все становится несколько сложнее.
    // Так как получить мы можем лишь указатель на начало буфера и непонятно каким же
    // образом мы получим размер выделеной памяти.
    // Но мы не зря написали `toy_alloc` таким образом, чтобы первые его 4 байта содержали
    // размер выделенного буфера.
    pub fn from_raw_ptr(instance: &'a Instance, offset: i32) -> Self {
        let offset = offset as usize;
        let memory = Self::get_memory(instance);

        let len = unsafe {
            // Получаем сырой указатель на память инстанса.
            let buf = memory.data_unchecked();

            let mut len_bytes = [0; WASM_PTR_LEN];
            // Читаем байты с размером выделенного буфера.
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

    // Методы для чтения и записи данных являются весьма тривиальными.
    // Важно лишь помнить про то, что нужно читать со смещением в 4 байта.

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
}
```

Важно не забывать вызывать деструктор, который будет очищать выделенную память.

```rust
impl Drop for WasmBuf<'_> {
    fn drop(&mut self) {
        Self::toy_free(self.instance, self.len);
    }
}
```

## Собираем все вместе

И вот теперь мы можем спокойно написать наш искомый процедурный макрос, который будет 
использовать WASM модули для расширения функциональности, без необходимости перекомпиляции.

```rust
#[proc_macro_derive(TextMessage, attributes(text_message))]
pub fn text_message(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input);

    let attrs =
        TextMessageAttrs::from_raw(&input.attrs).expect("Unable to parse text message attributes.");

    // Для простоты будем грузить модули из директории codecs, которые имеют особым образом
    // сформированное имя. 
    let codec_dir = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("codecs");
    let plugin_name = format!("{}_text_codec.wasm", attrs.codec);
    let codec_path = codec_dir.join(plugin_name);

    let wasm_macro = WasmMacro::from_file(codec_path).expect("Unable to load wasm module");
    wasm_macro
        .proc_macro_derive(
            "implement_codec",
            input.into_token_stream().into(),
        )
        .expect("Unable to apply proc_macro_attribute")
}
```

В [репозитории] есть готовый пример с демонстрацией работы. Вы можете убедиться, что кодек
действительно загружается из WASM модуля, а не компилируется вместе с макросом.

```rust
#[derive(Debug, Serialize, Deserialize, PartialEq, TextMessage)]
// Что особенно хорошо, каждый WASM плагин может иметь свои произвольные атрибуты.
#[text_message(codec = "serde_json", params(pretty))]
struct FooMessage {
    name: String,
    description: String,
    value: u64,
}

fn main() {
    let msg = FooMessage {
        name: "Linus Torvalds".to_owned(),
        description: "The Linux founder.".to_owned(),
        value: 1,
    };

    let text = msg.to_string();
    println!("{}", text);
    let msg2 = text.parse().unwrap();

    assert_eq!(msg, msg2);
}
```

## Выводы

Пока это больше похоже на троллейбус из буханки хлеба, но с другой стороны это небольшая, но
прекрасная демонстрация самого принципа. Такие макросы становятся открытыми для расширения. 
У нас больше нет необходимости в переписывании исходного процедурного макроса, чтобы изменить или
расширить его поведение.
А если же воспользоваться [реестром модулей] для WASM, то можно будет распространять подобные
модули подобно крейтам cargo.

[watt]: https://github.com/dtolnay/watt
[libloading]: https://crates.io/crates/libloading
[`syn`]: https://crates.io/crates/syn
[`quote`]: https://crates.io/crates/quote
[`proc-macro2`]: https://crates.io/crates/proc-macro2
[bytecodealliance]: https://bytecodealliance.org/
[интерфейс модуля]: https://github.com/bytecodealliance/wasmtime/tree/master/crates/misc/rust
[`wasm_bindgen`]: https://habr.com/en/post/353230/
[репозитории]: https://github.com/alekseysidorov/proc-macro-plugin/tree/master
[реестром модулей]: https://wapm.io/