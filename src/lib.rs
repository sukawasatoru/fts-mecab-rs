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

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use libc::{c_char, c_int, c_uchar, c_uint};
use std::ffi::{c_void, CStr, CString};
use std::fmt::{Display, Formatter};
use std::ptr::null_mut;
use tracing::{debug, info, warn};

// https://www.sqlite.org/loadext.html#programming_loadable_extensions
// SQLITE_EXTENSION_INIT1
// https://github.com/rusqlite/rusqlite/issues/524
#[no_mangle]
pub static mut sqlite3_api: *mut sqlite3_api_routines = null_mut();

/// An extension loading entry point.
///
/// https://www.sqlite.org/loadext.html#programming_loadable_extensions
#[no_mangle]
#[tracing::instrument(skip_all)]
pub unsafe extern "C" fn sqlite3_ftsmecabrs_init(
    db: *mut sqlite3,
    pzErrMsg: *mut *mut c_char,
    pApi: *const sqlite3_api_routines,
) -> c_int {
    if cfg!(feature = "tracing-subscriber") {
        tracing_subscriber::fmt::try_init().ok();
    }

    info!("hello");

    // SQLITE_EXTENSION_INIT2(pApi)
    sqlite3_api = pApi.cast_mut();

    let fts5api = fts5_api_from_db(db);

    if fts5api.is_null() {
        warn!("failed to initialize the fts5_api");
        let msg = CString::new("libfts_mecab_rs: %s").unwrap();
        *pzErrMsg =
            (*sqlite3_api).mprintf.unwrap()(msg.as_ptr(), (*sqlite3_api).errmsg.unwrap()(db));
        return SQLITE_ERROR as c_int;
    }

    let name = CString::new("mecab").unwrap();

    (*fts5api).xCreateTokenizer.unwrap()(
        fts5api,
        name.as_ptr(),
        fts5api.cast(),
        &mut fts5_tokenizer {
            xCreate: Some(create),
            xDelete: Some(delete),
            xTokenize: Some(tokenize),
        },
        None,
    )
}

struct StatementWrapper(*mut sqlite3_stmt);

impl Default for StatementWrapper {
    fn default() -> Self {
        Self(null_mut())
    }
}

impl Drop for StatementWrapper {
    fn drop(&mut self) {
        unsafe {
            (*sqlite3_api).finalize.unwrap()(self.0);
        }
    }
}

/// https://www.sqlite.org/fts5.html#extending_fts5
unsafe fn fts5_api_from_db(db: *mut sqlite3) -> *mut fts5_api {
    info!("fts5_api_from_db");

    let mut statement = StatementWrapper::default();

    let sql = CString::new("SELECT fts5(?1)").unwrap();
    let ret_prepare =
        (*sqlite3_api).prepare.unwrap()(db, sql.as_ptr(), -1, &mut statement.0, null_mut());

    if ret_prepare as u32 != SQLITE_OK {
        warn!(%ret_prepare, "prepare failed");
        return null_mut();
    };

    let mut fts5api = null_mut();
    let arg4 = CString::new("fts5_api_ptr").unwrap();
    (*sqlite3_api).bind_pointer.unwrap()(
        statement.0,
        1,
        &mut fts5api as *mut *mut _ as *mut c_void,
        arg4.as_ptr(),
        None,
    );
    (*sqlite3_api).step.unwrap()(statement.0);

    fts5api
}

struct TokenizerContext {
    mecab: *mut mecab_t,
}

impl Drop for TokenizerContext {
    fn drop(&mut self) {
        debug!("drop TokenizerContext");

        if !self.mecab.is_null() {
            unsafe {
                mecab_destroy(self.mecab);
            }
        }
    }
}

/// Initialize [Fts5Tokenizer] for table.
///
/// https://www.sqlite.org/fts5.html#custom_tokenizers
#[tracing::instrument(skip_all)]
unsafe extern "C" fn create(
    arg1: *mut c_void,
    azArg: *mut *const c_char,
    nArg: c_int,
    ppOut: *mut *mut Fts5Tokenizer,
) -> c_int {
    // called per table.
    info!("create");

    for i in 0..nArg {
        let value = CStr::from_ptr(*azArg.offset(i as isize));
        debug!(%i, ?value);
    }

    // let fts5api = arg1.cast::<fts5_api>();
    let context = Box::new(TokenizerContext {
        mecab: mecab_new(nArg, azArg.cast()),
    });

    let mut dic_info = mecab_dictionary_info(context.mecab);
    while !dic_info.is_null() {
        let filename = CStr::from_ptr((*dic_info).filename);
        let charset = CStr::from_ptr((*dic_info).charset);
        let size = (*dic_info).size;
        let version = (*dic_info).version;

        info!(?filename, ?charset, %size, %version);
        dic_info = (*dic_info).next;
    }
    drop(dic_info);

    *ppOut = Box::into_raw(context).cast();

    SQLITE_OK as c_int
}

/// Dispose [Fts5Tokenizer].
///
/// https://www.sqlite.org/fts5.html#custom_tokenizers
#[tracing::instrument(skip_all)]
unsafe extern "C" fn delete(arg1: *mut Fts5Tokenizer) {
    // called per table.
    let ctx = Box::from_raw(arg1.cast::<TokenizerContext>());
    drop(ctx);
    info!("bye");
}

/// https://www.sqlite.org/fts5.html#custom_tokenizers
#[tracing::instrument(skip_all)]
unsafe extern "C" fn tokenize(
    arg1: *mut Fts5Tokenizer,
    pCtx: *mut c_void,
    flags: c_int,
    pText: *const c_char,
    nText: c_int,
    xToken: Option<
        unsafe extern "C" fn(
            pCtx: *mut c_void,
            tflags: c_int,
            pToken: *const c_char,
            nToken: c_int,
            iStart: c_int,
            iEnd: c_int,
        ) -> c_int,
    >,
) -> c_int {
    info!(%flags, "tokenize");

    let context = arg1.cast::<TokenizerContext>();
    let x_token = xToken.unwrap();

    let text = CStr::from_ptr(pText);
    let text = text.to_str().unwrap();
    debug!(%text);

    let mut node = mecab_sparse_tonode2((*context).mecab, pText, nText as usize);

    if let Err(err) = check((*context).mecab, node.cast()) {
        warn!(?err, "node is null");
        return SQLITE_ERROR as c_int;
    }

    // http://taku910.github.io/mecab/libmecab.html
    // https://github.com/taku910/mecab/blob/046fa78/mecab/example/example.c#L38-L45
    let mut i_start = 0;
    while !node.is_null() {
        let stat = NodeStat::try_from((*node).stat).unwrap();
        match stat {
            NodeStat::BOS | NodeStat::EOS => {
                node = (*node).next;
                continue;
            }
            _ => {}
        }

        let length = (*node).length as c_int;
        let feature = CStr::from_ptr((*node).feature);
        let feature = feature.to_str().unwrap();
        let surface = {
            let mut buf = Vec::<u8>::with_capacity(length as usize);
            let surface_ptr = (*node).surface;
            for i in 0..length {
                buf.push(*surface_ptr.offset(i as isize) as u8);
            }
            CString::from_vec_unchecked(buf)
        };
        let surface_str = surface.to_str().unwrap();
        debug!(%stat, %feature, %length, ?surface_str);

        let ret = x_token(pCtx, 0, surface.as_ptr(), length, i_start, i_start + length);

        if ret != SQLITE_OK as i32 {
            return ret;
        }

        i_start += length;
        node = (*node).next;
    }

    SQLITE_OK as i32
}

unsafe fn check(mecab: *mut mecab_t, value: *const c_void) -> Result<(), CString> {
    if value.is_null() {
        Err(CStr::from_ptr(mecab_strerror(mecab)).into())
    } else {
        Ok(())
    }
}

#[allow(clippy::upper_case_acronyms)]
enum NodeStat {
    NOR,
    UNK,
    BOS,
    EOS,
    EON,
}

impl TryFrom<c_uchar> for NodeStat {
    type Error = c_uchar;

    fn try_from(value: c_uchar) -> Result<Self, Self::Error> {
        match value as c_uint {
            MECAB_NOR_NODE => Ok(Self::NOR),
            MECAB_UNK_NODE => Ok(Self::UNK),
            MECAB_BOS_NODE => Ok(Self::BOS),
            MECAB_EOS_NODE => Ok(Self::EOS),
            MECAB_EON_NODE => Ok(Self::EON),
            _ => Err(value),
        }
    }
}

impl Display for NodeStat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let data = match self {
            NodeStat::NOR => "MECAB_NOR_NODE",
            NodeStat::UNK => "MECAB_UNK_NODE",
            NodeStat::BOS => "MECAB_BOS_NODE",
            NodeStat::EOS => "MECAB_EOS_NODE",
            NodeStat::EON => "MECAB_EON_NODE",
        };
        f.write_str(data)
    }
}
