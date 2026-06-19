#![allow(clippy::result_large_err)]

//! # nom-kconfig
//!
//! A parser for kconfig files. The parsing is done with [nom](https://github.com/rust-bakery/nom).
//!
//! ```no_run
//! use std::path::PathBuf;
//! use nom_kconfig::{parse_kconfig, KconfigInput, KconfigFile};
//! use std::collections::HashMap;
//!
//! // curl https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-6.4.9.tar.xz | tar -xJ -C /tmp/
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut variables = HashMap::new();
//!     variables.insert("SRCARCH", "x86");
//!     let kconfig_file = KconfigFile::new_with_vars(
//!         PathBuf::from("/tmp/linux-6.4.9"),
//!         PathBuf::from("/tmp/linux-6.4.9/Kconfig"),
//!         &variables,
//!         &HashMap::default(),
//!     );
//!     let input = kconfig_file.read_to_string().unwrap();
//!     let kconfig = parse_kconfig(KconfigInput::new_extra(&input, kconfig_file));
//!     println!("{:?}", kconfig);
//!     Ok(())
//! }
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::{fs, io};

/// Make-style preprocessor variables shared across all parser input clones.
///
/// `global` holds variables fixed at parse startup (e.g. `SRCARCH=x86`).
/// `local` accumulates variable assignments encountered during parsing.
/// Both fields are `Rc`-wrapped so cloning `VarTable` is O(1) — nom clones
/// the parser input constantly for backtracking. `local` uses `RefCell` for
/// in-place writes through a shared reference.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VarTable {
    global: Rc<HashMap<String, String>>,
    local: Rc<RefCell<HashMap<String, String>>>,
}

impl VarTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_maps(global: HashMap<String, String>, local: HashMap<String, String>) -> Self {
        Self {
            global: Rc::new(global),
            local: Rc::new(RefCell::new(local)),
        }
    }

    /// Insert a variable into the live (local) table.
    pub fn insert(&self, key: impl Into<String>, value: impl Into<String>) {
        self.local.borrow_mut().insert(key.into(), value.into());
    }

    /// Extend the live (local) table with multiple variables.
    pub fn extend(&self, vars: impl IntoIterator<Item = (String, String)>) {
        self.local.borrow_mut().extend(vars);
    }

    /// Merged view of all variables; local values win over global on conflict.
    pub fn all(&self) -> HashMap<String, String> {
        let mut out = (*self.global).clone();
        out.extend(self.local.borrow().clone());
        out
    }

    pub fn global(&self) -> &HashMap<String, String> {
        &self.global
    }

    pub fn set_global(&mut self, vars: HashMap<String, String>) {
        self.global = Rc::new(vars);
    }
}

/// Represents a Kconfig file.
/// It stores the kernel root directory because we need this information when a [`source`](https://www.kernel.org/doc/html/next/kbuild/kconfig-language.html#kconfig-syntax) keyword is met.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct KconfigFile {
    /// The absolute path of the kernel root directory. This field is necessary to parse [`source`](https://www.kernel.org/doc/html/next/kbuild/kconfig-language.html#kconfig-syntax) entry.
    pub root_dir: PathBuf,
    /// The path the the Kconfig you want to parse.
    pub file: PathBuf,
    /// All variables, both global (defined externally), and local.
    pub vars: VarTable,
    pub external_functions: Rc<HashMap<String, String>>,
    pub depth: usize,
    pub parent_file: Option<PathBuf>,
}

impl KconfigFile {
    pub fn new(root_dir: PathBuf, file: PathBuf) -> Self {
        Self {
            root_dir,
            file,
            vars: VarTable::new(),
            external_functions: Rc::new(HashMap::new()),
            depth: 0,
            parent_file: None,
        }
    }

    pub fn new_with_vars<S: AsRef<str>>(
        root_dir: PathBuf,
        file: PathBuf,
        global_vars: &HashMap<S, S>,
        local_vars: &HashMap<S, S>,
    ) -> Self {
        Self {
            root_dir,
            file,
            vars: VarTable::from_maps(
                global_vars
                    .iter()
                    .map(|(s1, s2)| (s1.as_ref().to_string(), s2.as_ref().to_string()))
                    .collect(),
                local_vars
                    .iter()
                    .map(|(s1, s2)| (s1.as_ref().to_string(), s2.as_ref().to_string()))
                    .collect(),
            ),
            external_functions: Rc::new(HashMap::new()),
            depth: 0,
            parent_file: None,
        }
    }

    pub fn with_external_functions(mut self, external_functions: &HashMap<String, String>) -> Self {
        self.external_functions = Rc::new(external_functions.clone());
        self
    }

    pub fn new_source_file(&self, path: PathBuf) -> Self {
        let mut copied = self.clone();
        copied.file = path;
        copied.depth += 1;
        copied.parent_file = Some(self.file.clone());
        copied
    }

    pub fn set_global_vars<S: AsRef<str>>(&mut self, vars: &[(S, S)]) {
        self.vars.set_global(
            vars.iter()
                .map(|(s1, s2)| (s1.as_ref().to_string(), s2.as_ref().to_string()))
                .collect(),
        );
    }

    pub fn full_path(&self) -> PathBuf {
        self.root_dir.join(&self.file)
    }

    pub fn read_to_string(&self) -> io::Result<String> {
        fs::read_to_string(self.full_path()).map(|content| self.preprocess_content(content))
    }

    pub fn preprocess_content(&self, content: String) -> String {
        let variables = self.vars.all();
        if variables.is_empty() {
            return content;
        }
        let mut file_copy = content.clone();
        for (var_name, var_value) in variables {
            file_copy = file_copy.replace(&format!("$({var_name})"), &var_value);
            file_copy = file_copy.replace(&format!("${{{var_name}}}"), &var_value);
        }

        file_copy
    }
}
