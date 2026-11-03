//! Tiny hand-written syntax highlighter (no regex engine, no dependencies).
//! One pass per visible line; multi-line state (block comments, HTML <script>,
//! Python triple quotes, Markdown fences) is cached by the editor.

use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tok {
    Normal,
    Keyword,
    Type,
    Func,
    Str,
    Comment,
    Number,
    Const,
    Macro,
    Tag,
    Attr,
    Var,
}

/// Highlighter state carried from one line to the next.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct State {
    outer: u8, // HTML: 0 text, 1 <script>, 2 <style>, 3 <!-- comment -->
    inner: u8, // 0 none, 1 block comment / md fence, 2 """ string, 3 ''' string
}

const O_SCRIPT: u8 = 1;
const O_STYLE: u8 = 2;
const O_HCOMMENT: u8 = 3;
const I_BLOCK: u8 = 1;
const I_TRIPLE_D: u8 = 2;
const I_TRIPLE_S: u8 = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Plain,
    Generic,
    Html,
    Markdown,
}

pub struct Lang {
    pub name: &'static str,
    pub kind: Kind,
    exts: &'static [&'static str],
    line_c: &'static [&'static str],
    block: Option<(&'static str, &'static str)>,
    kw: &'static [&'static str],
    ty: &'static [&'static str],
    consts: &'static [&'static str],
    fn_kw: &'static [&'static str],
    type_kw: &'static [&'static str],
    var_kw: &'static [&'static str],
    quotes: &'static str,
    triple: bool,
    dollar: bool,
    macro_bang: bool,
    lifetimes: bool,
    attr_hash: bool,
    preproc: bool,
    at_attr: bool,
    hash_color: bool,
    json_keys: bool,
    line_keys: bool,
    section: bool,
    dash: bool,
    pub hard_tabs: bool,
}

impl Lang {
    /// Does highlighting depend on previous lines?
    pub fn multiline(&self) -> bool {
        self.block.is_some() || self.triple || matches!(self.kind, Kind::Html | Kind::Markdown)
    }
}

const BASE: Lang = Lang {
    name: "Plain Text",
    kind: Kind::Generic,
    exts: &[],
    line_c: &[],
    block: None,
    kw: &[],
    ty: &[],
    consts: &[],
    fn_kw: &[],
    type_kw: &[],
    var_kw: &[],
    quotes: "",
    triple: false,
    dollar: false,
    macro_bang: false,
    lifetimes: false,
    attr_hash: false,
    preproc: false,
    at_attr: false,
    hash_color: false,
    json_keys: false,
    line_keys: false,
    section: false,
    dash: false,
    hard_tabs: false,
};

pub static PLAIN: Lang = Lang { kind: Kind::Plain, ..BASE };

pub static RUST: Lang = Lang {
    name: "Rust",
    exts: &["rs"],
    line_c: &["//"],
    block: Some(("/*", "*/")),
    kw: &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self",
        "static", "struct", "super", "trait", "type", "unsafe", "use", "where", "while", "union",
    ],
    ty: &[
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64",
        "bool", "char", "str", "String", "Vec", "Option", "Result", "Box", "Self",
    ],
    consts: &["true", "false", "None", "Some", "Ok", "Err"],
    fn_kw: &["fn"],
    type_kw: &["struct", "enum", "trait", "type", "impl", "mod", "union"],
    var_kw: &["let", "mut", "static", "const"],
    quotes: "\"'",
    lifetimes: true,
    macro_bang: true,
    attr_hash: true,
    ..BASE
};

pub static PYTHON: Lang = Lang {
    name: "Python",
    exts: &["py", "pyw"],
    line_c: &["#"],
    kw: &[
        "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else",
        "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal", "not",
        "or", "pass", "raise", "return", "try", "while", "with", "yield",
    ],
    ty: &["int", "float", "str", "bool", "list", "dict", "set", "tuple", "bytes", "object", "self", "cls"],
    consts: &["True", "False", "None"],
    fn_kw: &["def"],
    type_kw: &["class"],
    quotes: "\"'",
    triple: true,
    at_attr: true,
    ..BASE
};

pub static JS: Lang = Lang {
    name: "JavaScript/TypeScript",
    exts: &["js", "mjs", "cjs", "jsx", "ts", "tsx"],
    line_c: &["//"],
    block: Some(("/*", "*/")),
    kw: &[
        "break", "case", "catch", "class", "const", "continue", "debugger", "default", "delete", "do", "else",
        "export", "extends", "finally", "for", "function", "if", "import", "in", "instanceof", "let", "new",
        "of", "return", "static", "super", "switch", "this", "throw", "try", "typeof", "var", "void", "while",
        "with", "yield", "async", "await", "interface", "type", "enum", "implements", "namespace", "declare",
        "readonly", "public", "private", "protected", "abstract", "from", "as",
    ],
    ty: &[
        "string", "number", "boolean", "any", "unknown", "never", "object", "Array", "Promise", "Map", "Set",
        "Date", "Math", "JSON", "Object", "console", "document", "window",
    ],
    consts: &["true", "false", "null", "undefined", "NaN", "Infinity"],
    fn_kw: &["function"],
    type_kw: &["class", "interface", "enum", "type", "namespace"],
    var_kw: &["let", "var", "const"],
    quotes: "\"'`",
    ..BASE
};

pub static C: Lang = Lang {
    name: "C/C++",
    exts: &["c", "h", "cpp", "cc", "cxx", "hpp", "hh"],
    line_c: &["//"],
    block: Some(("/*", "*/")),
    kw: &[
        "break", "case", "class", "const", "continue", "default", "delete", "do", "else", "enum", "extern",
        "for", "goto", "if", "inline", "namespace", "new", "private", "public", "protected", "return",
        "sizeof", "static", "struct", "switch", "template", "this", "typedef", "union", "using", "virtual",
        "while", "try", "catch", "throw", "constexpr", "typename",
    ],
    ty: &[
        "int", "char", "long", "short", "float", "double", "void", "unsigned", "signed", "bool", "size_t",
        "auto", "uint8_t", "uint16_t", "uint32_t", "uint64_t", "int8_t", "int16_t", "int32_t", "int64_t",
    ],
    consts: &["true", "false", "NULL", "nullptr"],
    type_kw: &["struct", "enum", "union", "class", "namespace"],
    quotes: "\"'",
    preproc: true,
    ..BASE
};

pub static JAVA: Lang = Lang {
    name: "Java/C#",
    exts: &["java", "cs", "kt", "swift", "dart"],
    line_c: &["//"],
    block: Some(("/*", "*/")),
    kw: &[
        "abstract", "break", "case", "catch", "class", "const", "continue", "default", "do", "else", "enum",
        "extends", "final", "finally", "for", "if", "implements", "import", "instanceof", "interface", "new",
        "package", "private", "protected", "public", "return", "static", "super", "switch", "this", "throw",
        "throws", "try", "var", "void", "while", "using", "namespace", "override", "fun", "val", "let", "func",
    ],
    ty: &["int", "long", "short", "byte", "float", "double", "boolean", "char", "String", "Object", "bool", "string"],
    consts: &["true", "false", "null", "nil"],
    type_kw: &["class", "interface", "enum"],
    var_kw: &["var", "val", "let"],
    fn_kw: &["fun", "func"],
    quotes: "\"'",
    at_attr: true,
    ..BASE
};

pub static GO: Lang = Lang {
    name: "Go",
    exts: &["go"],
    line_c: &["//"],
    block: Some(("/*", "*/")),
    kw: &[
        "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough", "for", "func",
        "go", "goto", "if", "import", "interface", "map", "package", "range", "return", "select", "struct",
        "switch", "type", "var",
    ],
    ty: &[
        "int", "int8", "int16", "int32", "int64", "uint", "uint8", "uint16", "uint32", "uint64", "string",
        "bool", "byte", "rune", "error", "float32", "float64",
    ],
    consts: &["true", "false", "nil", "iota"],
    fn_kw: &["func"],
    type_kw: &["type"],
    var_kw: &["var", "const"],
    quotes: "\"'`",
    hard_tabs: true,
    ..BASE
};

pub static SHELL: Lang = Lang {
    name: "Shell",
    exts: &["sh", "bash", "zsh", "fish", "env"],
    line_c: &["#"],
    kw: &[
        "if", "then", "else", "elif", "fi", "for", "while", "until", "do", "done", "case", "esac", "in",
        "function", "select", "time", "return", "exit", "break", "continue", "local", "export", "readonly",
        "unset", "alias", "source", "declare",
    ],
    consts: &["true", "false"],
    fn_kw: &["function"],
    quotes: "\"'",
    dollar: true,
    ..BASE
};

pub static CSS: Lang = Lang {
    name: "CSS",
    exts: &["css", "scss", "less"],
    block: Some(("/*", "*/")),
    consts: &["important", "inherit", "none", "auto"],
    quotes: "\"'",
    at_attr: true,
    hash_color: true,
    line_keys: true,
    dash: true,
    ..BASE
};

pub static JSON: Lang = Lang {
    name: "JSON",
    exts: &["json", "jsonc", "geojson"],
    consts: &["true", "false", "null"],
    quotes: "\"",
    json_keys: true,
    ..BASE
};

pub static CONFIG: Lang = Lang {
    name: "TOML/YAML/INI",
    exts: &["toml", "yaml", "yml", "ini", "cfg", "conf"],
    line_c: &["#"],
    consts: &["true", "false", "null", "yes", "no"],
    quotes: "\"'",
    line_keys: true,
    section: true,
    dash: true,
    ..BASE
};

pub static HTML: Lang = Lang {
    name: "HTML/XML",
    kind: Kind::Html,
    exts: &["html", "htm", "xhtml", "xml", "svg", "vue", "xaml"],
    ..BASE
};

pub static MARKDOWN: Lang = Lang {
    name: "Markdown",
    kind: Kind::Markdown,
    exts: &["md", "markdown"],
    ..BASE
};

pub static LUA: Lang = Lang {
    name: "Lua",
    exts: &["lua"],
    line_c: &["--"],
    block: Some(("--[[", "]]")),
    kw: &[
        "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto",
        "if", "in", "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
    ],
    ty: &["table", "string", "number", "boolean", "thread", "userdata"],
    consts: &["true", "false", "nil"],
    fn_kw: &["function"],
    var_kw: &["local"],
    quotes: "\"'",
    ..BASE
};

pub static RUBY: Lang = Lang {
    name: "Ruby",
    exts: &["rb", "rake", "gemspec", "ru"],
    line_c: &["#"],
    block: Some(("=begin", "=end")),
    kw: &[
        "alias", "and", "begin", "break", "case", "class", "def", "defined?", "do",
        "else", "elsif", "end", "ensure", "false", "for", "if", "in", "module", "next",
        "nil", "not", "or", "redo", "rescue", "retry", "return", "self", "super", "then",
        "true", "undef", "unless", "until", "when", "while", "yield",
    ],
    ty: &["Integer", "Float", "String", "Array", "Hash", "Symbol", "Regexp", "Range"],
    consts: &["true", "false", "nil"],
    fn_kw: &["def"],
    type_kw: &["class", "module"],
    quotes: "\"'",
    ..BASE
};

pub static SQL: Lang = Lang {
    name: "SQL",
    exts: &["sql"],
    line_c: &["--"],
    block: Some(("/*", "*/")),
    kw: &[
        "select", "insert", "update", "delete", "from", "where", "join", "left", "right",
        "inner", "outer", "on", "group", "by", "order", "having", "limit", "offset",
        "create", "table", "drop", "alter", "add", "column", "primary", "key", "foreign",
        "references", "index", "view", "as", "and", "or", "not", "in", "is", "null",
        "like", "between", "union", "all", "distinct", "case", "when", "then", "else",
        "end", "into", "values", "set", "default", "SELECT", "INSERT", "UPDATE", "DELETE",
        "FROM", "WHERE", "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "ON", "GROUP", "BY",
        "ORDER", "HAVING", "LIMIT", "OFFSET", "CREATE", "TABLE", "DROP", "ALTER", "ADD",
        "COLUMN", "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "INDEX", "VIEW", "AS", "AND",
        "OR", "NOT", "IN", "IS", "NULL", "LIKE", "BETWEEN", "UNION", "ALL", "DISTINCT",
        "CASE", "WHEN", "THEN", "ELSE", "END", "INTO", "VALUES", "SET", "DEFAULT",
    ],
    ty: &[
        "int", "integer", "bigint", "smallint", "varchar", "char", "text", "boolean",
        "date", "time", "timestamp", "float", "double", "decimal", "numeric", "blob",
        "INT", "INTEGER", "BIGINT", "SMALLINT", "VARCHAR", "CHAR", "TEXT", "BOOLEAN",
        "DATE", "TIME", "TIMESTAMP", "FLOAT", "DOUBLE", "DECIMAL", "NUMERIC", "BLOB",
    ],
    consts: &["true", "false", "null", "TRUE", "FALSE", "NULL"],
    quotes: "\"'`",
    ..BASE
};

pub static ZIG: Lang = Lang {
    name: "Zig",
    exts: &["zig", "zon"],
    line_c: &["//"],
    kw: &[
        "const", "var", "fn", "pub", "return", "if", "else", "switch", "while", "for",
        "break", "continue", "struct", "enum", "union", "error", "try", "catch", "orelse",
        "unreachable", "async", "await", "suspend", "resume", "defer", "errdefer", "test",
        "comptime", "inline", "extern", "export",
    ],
    ty: &[
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
        "f16", "f32", "f64", "f80", "f128", "bool", "void", "anyerror", "type",
    ],
    consts: &["true", "false", "null", "undefined"],
    fn_kw: &["fn"],
    type_kw: &["struct", "enum", "union", "error"],
    var_kw: &["const", "var"],
    quotes: "\"'",
    ..BASE
};

static ALL: &[&Lang] = &[
    &RUST, &PYTHON, &JS, &C, &JAVA, &GO, &SHELL, &CSS, &JSON, &CONFIG, &HTML, &MARKDOWN,
    &LUA, &RUBY, &SQL, &ZIG,
];

pub fn detect(path: Option<&Path>) -> &'static Lang {
    let Some(p) = path else { return &PLAIN };
    if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
        let ext = ext.to_ascii_lowercase();
        for l in ALL {
            if l.exts.contains(&ext.as_str()) {
                return l;
            }
        }
    }
    &PLAIN
}

// ---------------------------------------------------------------------------

pub fn highlight(lang: &Lang, line: &[char], st: &mut State, out: &mut Vec<Tok>) {
    out.clear();
    out.resize(line.len(), Tok::Normal);
    match lang.kind {
        Kind::Plain => {}
        Kind::Generic => generic(lang, line, out, &mut st.inner),
        Kind::Html => html(line, out, st),
        Kind::Markdown => markdown(line, out, &mut st.inner),
    }
}

fn fill(out: &mut [Tok], a: usize, b: usize, t: Tok) {
    let b = b.min(out.len());
    if a < b {
        for x in &mut out[a..b] {
            *x = t;
        }
    }
}

fn starts(line: &[char], i: usize, pat: &str) -> bool {
    let mut k = i;
    for c in pat.chars() {
        if k >= line.len() || line[k] != c {
            return false;
        }
        k += 1;
    }
    true
}

fn find_pat(line: &[char], from: usize, pat: &str) -> Option<usize> {
    (from..line.len()).find(|&i| starts(line, i, pat))
}

fn starts_ci(line: &[char], i: usize, pat: &str) -> bool {
    let mut k = i;
    for c in pat.chars() {
        if k >= line.len() || line[k].to_ascii_lowercase() != c {
            return false;
        }
        k += 1;
    }
    true
}

fn ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn ident_char(c: char, dash: bool) -> bool {
    c.is_alphanumeric() || c == '_' || (dash && c == '-')
}

fn next_non_space(line: &[char], mut i: usize) -> Option<char> {
    while i < line.len() {
        if !line[i].is_whitespace() {
            return Some(line[i]);
        }
        i += 1;
    }
    None
}

fn is_all_caps(w: &str) -> bool {
    w.chars().count() >= 2
        && w.chars().any(|c| c.is_alphabetic())
        && w.chars().all(|c| c.is_uppercase() || c.is_ascii_digit() || c == '_')
}

#[derive(Clone, Copy, PartialEq)]
enum Prev {
    None,
    Fn,
    Ty,
    Var,
}

fn generic(lang: &Lang, line: &[char], out: &mut [Tok], inner: &mut u8) {
    let n = line.len();
    let mut i = 0;

    // Continue a construct from the previous line.
    match *inner {
        I_BLOCK => {
            let end = lang.block.map(|b| b.1).unwrap_or("");
            match find_pat(line, 0, end) {
                Some(p) => {
                    let e = p + end.chars().count();
                    fill(out, 0, e, Tok::Comment);
                    *inner = 0;
                    i = e;
                }
                None => {
                    fill(out, 0, n, Tok::Comment);
                    return;
                }
            }
        }
        I_TRIPLE_D | I_TRIPLE_S => {
            let q = if *inner == I_TRIPLE_D { "\"\"\"" } else { "'''" };
            match find_pat(line, 0, q) {
                Some(p) => {
                    fill(out, 0, p + 3, Tok::Str);
                    *inner = 0;
                    i = p + 3;
                }
                None => {
                    fill(out, 0, n, Tok::Str);
                    return;
                }
            }
        }
        _ => {}
    }

    let lead = line.iter().position(|c| !c.is_whitespace()).unwrap_or(n);
    if lang.section && lead < n && line[lead] == '[' {
        fill(out, lead, n, Tok::Type);
        return;
    }

    let mut prev = Prev::None;
    while i < n {
        let c = line[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let old = prev;
        prev = Prev::None;

        // line comment
        if lang.line_c.iter().any(|lc| starts(line, i, lc)) {
            fill(out, i, n, Tok::Comment);
            return;
        }
        // block comment
        if let Some((s, e)) = lang.block {
            if starts(line, i, s) {
                let sl = s.chars().count();
                match find_pat(line, i + sl, e) {
                    Some(p) => {
                        let end = p + e.chars().count();
                        fill(out, i, end, Tok::Comment);
                        i = end;
                        continue;
                    }
                    None => {
                        fill(out, i, n, Tok::Comment);
                        *inner = I_BLOCK;
                        return;
                    }
                }
            }
        }
        // triple-quoted strings
        if lang.triple && (starts(line, i, "\"\"\"") || starts(line, i, "'''")) {
            let dq = line[i] == '"';
            let q = if dq { "\"\"\"" } else { "'''" };
            match find_pat(line, i + 3, q) {
                Some(p) => {
                    fill(out, i, p + 3, Tok::Str);
                    i = p + 3;
                    continue;
                }
                None => {
                    fill(out, i, n, Tok::Str);
                    *inner = if dq { I_TRIPLE_D } else { I_TRIPLE_S };
                    return;
                }
            }
        }
        // Rust attributes  #[...]  #![...]
        if lang.attr_hash && c == '#' && (starts(line, i, "#[") || starts(line, i, "#![")) {
            let end = line[i..].iter().position(|&x| x == ']').map_or(n, |p| i + p + 1);
            fill(out, i, end, Tok::Macro);
            i = end;
            continue;
        }
        // C preprocessor
        if lang.preproc && c == '#' && i == lead {
            let mut j = i + 1;
            while j < n && line[j].is_whitespace() {
                j += 1;
            }
            while j < n && ident_char(line[j], false) {
                j += 1;
            }
            fill(out, i, j, Tok::Macro);
            i = j;
            continue;
        }
        // decorators / at-rules
        if lang.at_attr && c == '@' && i + 1 < n && ident_start(line[i + 1]) {
            let mut j = i + 1;
            while j < n && (ident_char(line[j], lang.dash) || line[j] == '.') {
                j += 1;
            }
            fill(out, i, j, Tok::Macro);
            i = j;
            continue;
        }
        // CSS hex colours
        if lang.hash_color && c == '#' && i + 1 < n && line[i + 1].is_ascii_hexdigit() {
            let mut j = i + 1;
            while j < n && line[j].is_ascii_hexdigit() {
                j += 1;
            }
            fill(out, i, j, Tok::Number);
            i = j;
            continue;
        }
        // shell variables  $VAR  ${VAR}  $1  $?
        if lang.dollar && c == '$' {
            let mut j = i + 1;
            if j < n && line[j] == '{' {
                while j < n && line[j] != '}' {
                    j += 1;
                }
                j = (j + 1).min(n);
            } else if j < n && ident_start(line[j]) {
                while j < n && ident_char(line[j], false) {
                    j += 1;
                }
            } else if j < n {
                j += 1;
            }
            fill(out, i, j, Tok::Var);
            i = j;
            continue;
        }
        // strings
        if lang.quotes.contains(c) {
            if c == '\'' && lang.lifetimes {
                let is_char = (i + 2 < n && line[i + 2] == '\'') || (i + 1 < n && line[i + 1] == '\\');
                if !is_char {
                    let mut j = i + 1;
                    while j < n && ident_char(line[j], false) {
                        j += 1;
                    }
                    fill(out, i, j, Tok::Type);
                    i = j;
                    continue;
                }
            }
            let s = i;
            i += 1;
            while i < n {
                if line[i] == '\\' {
                    i += 2;
                    continue;
                }
                if line[i] == c {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let e = i.min(n);
            let t = if lang.json_keys && next_non_space(line, e) == Some(':') { Tok::Var } else { Tok::Str };
            fill(out, s, e, t);
            i = e;
            continue;
        }
        // numbers
        if c.is_ascii_digit() || (c == '.' && i + 1 < n && line[i + 1].is_ascii_digit()) {
            let s = i;
            i += 1;
            while i < n
                && (line[i].is_alphanumeric()
                    || line[i] == '_'
                    || (line[i] == '.' && i + 1 < n && line[i + 1].is_ascii_digit()))
            {
                i += 1;
            }
            fill(out, s, i, Tok::Number);
            continue;
        }
        // identifiers / keywords
        if ident_start(c) {
            let s = i;
            i += 1;
            while i < n && ident_char(line[i], lang.dash) {
                i += 1;
            }
            while lang.dash && i > s + 1 && line[i - 1] == '-' {
                i -= 1; // don't swallow a trailing dash
            }
            let word: String = line[s..i].iter().collect();
            let w = word.as_str();
            let mut end = i;
            let tok;
            if lang.macro_bang && i < n && line[i] == '!' && !(i + 1 < n && line[i + 1] == '=') {
                tok = Tok::Macro;
                end = i + 1;
            } else if lang.kw.contains(&w) {
                prev = if lang.fn_kw.contains(&w) {
                    Prev::Fn
                } else if lang.type_kw.contains(&w) {
                    Prev::Ty
                } else if lang.var_kw.contains(&w) {
                    Prev::Var
                } else {
                    Prev::None
                };
                tok = Tok::Keyword;
            } else if lang.consts.contains(&w) {
                tok = Tok::Const;
            } else if lang.ty.contains(&w) {
                tok = Tok::Type;
            } else if is_all_caps(w) {
                tok = Tok::Const;
            } else {
                let nx = next_non_space(line, i);
                tok = match old {
                    Prev::Fn => Tok::Func,
                    Prev::Ty => Tok::Type,
                    Prev::Var => Tok::Var,
                    Prev::None => {
                        if lang.line_keys && s == lead && matches!(nx, Some(':') | Some('=')) {
                            Tok::Var
                        } else if nx == Some('(') {
                            Tok::Func
                        } else if w.chars().next().map_or(false, |f| f.is_uppercase())
                            && w.chars().any(|c| c.is_lowercase())
                        {
                            Tok::Type
                        } else {
                            Tok::Normal
                        }
                    }
                };
            }
            fill(out, s, end, tok);
            i = end;
            continue;
        }
        i += 1;
    }
}

fn html(line: &[char], out: &mut [Tok], st: &mut State) {
    let n = line.len();
    let mut i = 0;
    while i < n {
        match st.outer {
            O_HCOMMENT => {
                match find_pat(line, i, "-->") {
                    Some(p) => {
                        fill(out, i, p + 3, Tok::Comment);
                        i = p + 3;
                        st.outer = 0;
                    }
                    None => {
                        fill(out, i, n, Tok::Comment);
                        return;
                    }
                }
                continue;
            }
            O_SCRIPT | O_STYLE => {
                let close = if st.outer == O_SCRIPT { "</script" } else { "</style" };
                let end = (i..n).find(|&k| starts_ci(line, k, close)).unwrap_or(n);
                if end > i {
                    let lang: &Lang = if st.outer == O_SCRIPT { &JS } else { &CSS };
                    generic(lang, &line[i..end], &mut out[i..end], &mut st.inner);
                }
                if end < n {
                    st.outer = 0;
                    st.inner = 0;
                    i = end;
                    continue;
                }
                return;
            }
            _ => {}
        }

        if starts(line, i, "<!--") {
            st.outer = O_HCOMMENT;
            fill(out, i, i + 4, Tok::Comment);
            i += 4;
            continue;
        }
        let c = line[i];
        if c == '<' && i + 1 < n && (line[i + 1].is_alphabetic() || matches!(line[i + 1], '/' | '!' | '?')) {
            let opening = line[i + 1] != '/';
            let mut j = i + 1;
            if matches!(line[j], '/' | '!' | '?') {
                j += 1;
            }
            let ns = j;
            while j < n && (line[j].is_alphanumeric() || matches!(line[j], '-' | ':' | '_' | '.')) {
                j += 1;
            }
            fill(out, i, j, Tok::Tag);
            let name: String = line[ns..j].iter().map(|c| c.to_ascii_lowercase()).collect();
            let mut closed = false;
            let mut selfclose = false;
            while j < n {
                let ch = line[j];
                if ch == '>' {
                    fill(out, j, j + 1, Tok::Tag);
                    j += 1;
                    closed = true;
                    break;
                }
                if ch == '/' && j + 1 < n && line[j + 1] == '>' {
                    fill(out, j, j + 2, Tok::Tag);
                    j += 2;
                    closed = true;
                    selfclose = true;
                    break;
                }
                if ch == '"' || ch == '\'' {
                    let s = j;
                    j += 1;
                    while j < n && line[j] != ch {
                        j += 1;
                    }
                    if j < n {
                        j += 1;
                    }
                    fill(out, s, j, Tok::Str);
                    continue;
                }
                if ch.is_alphabetic() || matches!(ch, '_' | ':' | '@' | '-') {
                    let s = j;
                    while j < n && (line[j].is_alphanumeric() || matches!(line[j], '_' | ':' | '@' | '-' | '.')) {
                        j += 1;
                    }
                    fill(out, s, j, Tok::Attr);
                    continue;
                }
                j += 1;
            }
            i = j;
            if closed && opening && !selfclose {
                if name == "script" {
                    st.outer = O_SCRIPT;
                    st.inner = 0;
                } else if name == "style" {
                    st.outer = O_STYLE;
                    st.inner = 0;
                }
            }
            continue;
        }
        if c == '&' {
            let mut j = i + 1;
            while j < n && j < i + 10 && (line[j].is_alphanumeric() || line[j] == '#') {
                j += 1;
            }
            if j < n && line[j] == ';' && j > i + 1 {
                fill(out, i, j + 1, Tok::Const);
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
}

fn markdown(line: &[char], out: &mut [Tok], inner: &mut u8) {
    let n = line.len();
    let lead = line.iter().position(|c| !c.is_whitespace());
    if let Some(p) = lead {
        if starts(line, p, "```") || starts(line, p, "~~~") {
            *inner = if *inner == I_BLOCK { 0 } else { I_BLOCK };
            fill(out, 0, n, Tok::Comment);
            return;
        }
    }
    if *inner == I_BLOCK {
        fill(out, 0, n, Tok::Str);
        return;
    }
    let Some(p) = lead else { return };
    if line[p] == '#' {
        fill(out, 0, n, Tok::Keyword);
        return;
    }
    if line[p] == '>' {
        fill(out, 0, n, Tok::Comment);
        return;
    }
    let mut i = p;
    if matches!(line[p], '-' | '*' | '+') && p + 1 < n && line[p + 1] == ' ' {
        fill(out, p, p + 1, Tok::Number);
        i = p + 1;
    } else if line[p].is_ascii_digit() {
        let mut j = p;
        while j < n && line[j].is_ascii_digit() {
            j += 1;
        }
        if j < n && line[j] == '.' && j + 1 < n && line[j + 1] == ' ' {
            fill(out, p, j + 1, Tok::Number);
            i = j + 1;
        }
    }
    while i < n {
        match line[i] {
            '`' => {
                let e = (i + 1..n).find(|&k| line[k] == '`').map_or(n, |k| k + 1);
                fill(out, i, e, Tok::Str);
                i = e;
            }
            '*' if starts(line, i, "**") => {
                let e = find_pat(line, i + 2, "**").map_or(n, |k| k + 2);
                fill(out, i, e, Tok::Type);
                i = e;
            }
            '[' => {
                if let Some(close) = (i + 1..n).find(|&k| line[k] == ']') {
                    if close + 1 < n && line[close + 1] == '(' {
                        let e = (close + 2..n).find(|&k| line[k] == ')').map_or(n, |k| k + 1);
                        fill(out, i, close + 1, Tok::Func);
                        fill(out, close + 1, e, Tok::Str);
                        i = e;
                        continue;
                    }
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(lang: &Lang, s: &str) -> Vec<Tok> {
        let line: Vec<char> = s.chars().collect();
        let mut st = State::default();
        let mut out = Vec::new();
        highlight(lang, &line, &mut st, &mut out);
        out
    }

    #[test]
    fn rust_basics() {
        let t = run(&RUST, "let mut x = 42; // hi");
        assert_eq!(t[0], Tok::Keyword); // let
        assert_eq!(t[8], Tok::Var); // x
        assert_eq!(t[12], Tok::Number);
        assert_eq!(t[16], Tok::Comment);
    }

    #[test]
    fn rust_fn_and_macro_and_lifetime() {
        let t = run(&RUST, "fn go<'a>() { println!(\"s\"); }");
        assert_eq!(t[3], Tok::Func);
        assert_eq!(t[6], Tok::Type); // 'a
        assert_eq!(t[14], Tok::Macro);
    }

    #[test]
    fn html_tag_attr_string() {
        let t = run(&HTML, "<a href=\"x\">hi</a>");
        assert_eq!(t[1], Tok::Tag);
        assert_eq!(t[3], Tok::Attr);
        assert_eq!(t[8], Tok::Str);
        assert_eq!(t[13], Tok::Normal);
    }

    #[test]
    fn multiline_block_comment_state() {
        let mut st = State::default();
        let mut out = Vec::new();
        let a: Vec<char> = "x /* start".chars().collect();
        highlight(&RUST, &a, &mut st, &mut out);
        assert_eq!(st.inner, I_BLOCK);
        let b: Vec<char> = "still */ y".chars().collect();
        highlight(&RUST, &b, &mut st, &mut out);
        assert_eq!(out[0], Tok::Comment);
        assert_eq!(st.inner, 0);
    }

    #[test]
    fn html_script_uses_js() {
        let mut st = State::default();
        let mut out = Vec::new();
        let a: Vec<char> = "<script>".chars().collect();
        highlight(&HTML, &a, &mut st, &mut out);
        let b: Vec<char> = "const a = 1;".chars().collect();
        highlight(&HTML, &b, &mut st, &mut out);
        assert_eq!(out[0], Tok::Keyword);
    }

    #[test]
    fn detect_by_extension() {
        assert_eq!(detect(Some(Path::new("index.html"))).name, "HTML/XML");
        assert_eq!(detect(Some(Path::new("a.RS"))).name, "Rust");
        assert_eq!(detect(Some(Path::new("noext"))).name, "Plain Text");
    }
}
