//! `Db.escape_string` writes BYTES as a hex BLOB literal.
//!
//! lobsters' `comments.confidence_order` is a `t.binary` holding
//! `[a, b, c].pack("CCC")`. Quoted as text, a NUL byte ended the SQL
//! ("unrecognized token: \"'\"") on every comment insert, which stopped
//! 73 of its model specs; and a non-NUL binary value stored as TEXT
//! sorts before every BLOB, so the column would order wrong once it held
//! both. Bytes = any NUL, or a BINARY-encoded string that is not plain
//! ASCII. An ASCII-only BINARY string stays text, as it always was.
//!
//! All three ruby-family SQL-literal shims carry the rule: the gem-backed
//! one, the JDBC one, and the spinel FFI one (`runtime/spinel/db.rb`).
//! The method is evaluated alone, because loading the whole shim needs
//! either the sqlite3 gem (the gem-backed shim) or spinel's `ffi_func`
//! declarations (the FFI shim), neither of which the unit job provides.
//!
//! The FFI shim's stake is the sharper one: it calls
//! `sqlite3_prepare_v2(dbh, sql, -1, …)`, so the statement is a C string
//! and a NUL inside an escaped literal ends it mid-literal — the prepare
//! fails and a request whose parameter carried %00 answers 500, where
//! the gem-backed lanes answer the same request (their hex BLOB literal
//! compares TEXT-vs-BLOB and matches nothing). The contract test pins
//! the shared answer, so the lanes cannot drift apart again.

use std::process::Command;

fn check(shim: &str) {
    let script = format!(
        r##"
src = File.read("{shim}")
defn = src[/^  def self\.escape_string\(s\)\n.*?^  end\n/m] or abort "no escape_string in {shim}"
module Db; end
Db.module_eval(defn)
cases = {{
  [0, 0, 0].pack("CCC")   => "X'000000'",
  [200, 1, 7].pack("CCC") => "X'c80107'",
  "a\0b"                  => "X'610062'",
  "it's"                  => "'it''s'",
  "abc".b                 => "'abc'",
  "héllo"                 => "'héllo'",
  nil                     => "''",
}}
bad = cases.filter_map {{ |v, want| got = Db.escape_string(v); "#{{v.inspect}} -> #{{got}} (want #{{want}})" if got != want }}
print bad.empty? ? "OK" : bad.join("; ")
"##
    );
    let out = Command::new("ruby")
        .arg("-e")
        .arg(&script)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("ruby");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.trim(),
        "OK",
        "{shim}: stdout={stdout}\nstderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn the_cruby_shim_writes_bytes_as_a_blob_literal() {
    check("runtime/spinel/db_cruby.rb");
}

#[test]
fn the_jruby_shim_writes_bytes_as_a_blob_literal() {
    check("runtime/spinel/db_jruby.rb");
}

#[test]
fn the_spinel_ffi_shim_writes_bytes_as_a_blob_literal() {
    check("runtime/spinel/db.rb");
}
