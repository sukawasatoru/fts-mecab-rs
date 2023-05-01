/*
 * Copyright 2023 sukawasatoru
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(clippy::type_complexity)]
#![allow(clippy::upper_case_acronyms)]

use std::ptr::null_mut;
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// https://www.sqlite.org/loadext.html#programming_loadable_extensions
// SQLITE_EXTENSION_INIT1
// https://github.com/rusqlite/rusqlite/issues/524
#[no_mangle]
pub static mut sqlite3_api: *mut sqlite3_api_routines = null_mut();
