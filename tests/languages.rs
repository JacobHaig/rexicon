//! Per-language extraction tests.
//!
//! One test per supported language; each fixture exercises the language's
//! top-level declarations plus at least one nested case. Asserts on symbol
//! kinds and signatures returned by `extract_from_bytes`, which is the
//! filesystem-free core of the extractor.

use rexicon::symbol::{FileIndex, SymbolKind};
use rexicon::treesitter::extract_from_bytes;
use std::path::Path;

// -- helpers ----------------------------------------------------------------

/// Flat list of top-level signatures.
fn top_sigs(idx: &FileIndex) -> Vec<&str> {
    idx.symbols.iter().map(|s| s.signature.as_str()).collect()
}

/// Signatures of the children of the symbol whose signature matches
/// `parent_sig` exactly. Returns an empty vec if no parent matches.
fn nested_sigs<'a>(idx: &'a FileIndex, parent_sig: &str) -> Vec<&'a str> {
    idx.symbols
        .iter()
        .find(|s| s.signature == parent_sig)
        .map(|s| s.children.iter().map(|c| c.signature.as_str()).collect())
        .unwrap_or_default()
}

fn run(lang: &str, source: &str) -> FileIndex {
    extract_from_bytes(Path::new("fixture"), lang, source.as_bytes())
        .unwrap_or_else(|e| panic!("extract {} failed: {}", lang, e))
}

// -- Rust -------------------------------------------------------------------
#[test]
fn rust_extract() {
    let src = r#"
pub struct Foo {
    x: i32,
}

pub enum E {
    A,
    B(u32),
}

pub trait T {
    fn m(&self);
}

impl Foo {
    pub fn new() -> Self { Foo { x: 0 } }
    fn helper(&self) -> i32 { self.x }
}

pub const MAX: u32 = 42;
pub type Alias = Foo;

pub fn free() {}
"#;
    let idx = run("rust", src);
    let tops = top_sigs(&idx);
    assert!(tops.iter().any(|s| s.starts_with("pub struct Foo")));
    assert!(tops.iter().any(|s| s.starts_with("pub enum E")));
    assert!(tops.iter().any(|s| s.starts_with("pub trait T")));
    assert!(tops.iter().any(|s| s.starts_with("impl Foo")));
    assert!(
        tops.iter()
            .any(|s| s.contains("MAX") && s.contains("= ..."))
    );
    assert!(tops.iter().any(|s| s.starts_with("pub type Alias")));
    assert!(tops.iter().any(|s| s.starts_with("pub fn free()")));

    let impl_children = nested_sigs(&idx, "impl Foo { ... }");
    assert!(impl_children.iter().any(|s| s.contains("pub fn new")));
    assert!(impl_children.iter().any(|s| s.contains("fn helper")));

    let enum_children = nested_sigs(&idx, "pub enum E { ... }");
    assert!(enum_children.contains(&"A"));
    // Tuple variant body is elided to `{ ... }` so just check the name.
    assert!(enum_children.iter().any(|s| s.starts_with("B")));
}

// -- Python -----------------------------------------------------------------
#[test]
fn python_extract() {
    let src = r#"
X = 1

def free():
    return 1

class Foo:
    def method(self):
        return 2

    def other(self, x):
        return x
"#;
    let idx = run("python", src);
    let tops = top_sigs(&idx);
    assert!(tops.iter().any(|s| s.starts_with("def free")));
    assert!(tops.iter().any(|s| s.starts_with("class Foo")));

    let foo = idx
        .symbols
        .iter()
        .find(|s| s.signature.starts_with("class Foo"))
        .unwrap();
    let names: Vec<&str> = foo.children.iter().map(|c| c.signature.as_str()).collect();
    assert!(names.iter().any(|s| s.contains("method")));
    assert!(names.iter().any(|s| s.contains("other")));
}

// -- Go ---------------------------------------------------------------------
#[test]
fn go_extract() {
    let src = r#"
package main

type User struct {
    Name string
}

func (u *User) Greet() string {
    return "hi " + u.Name
}

func main() {}

const Pi = 3.14
"#;
    let idx = run("go", src);
    let tops = top_sigs(&idx);
    assert!(tops.iter().any(|s| s.contains("type User struct")));
    assert!(tops.iter().any(|s| s.contains("func (u *User) Greet()")));
    assert!(tops.iter().any(|s| s.contains("func main()")));
}

// -- C ----------------------------------------------------------------------
#[test]
fn c_extract() {
    let src = r#"
#include <stdio.h>

struct Point {
    int x;
    int y;
};

enum Color { RED, GREEN, BLUE };

int add(int a, int b) {
    return a + b;
}

static const int MAX = 100;
"#;
    let idx = run("c", src);
    let tops = top_sigs(&idx);
    assert!(tops.iter().any(|s| s.contains("struct Point")));
    assert!(tops.iter().any(|s| s.contains("enum Color")));
    assert!(tops.iter().any(|s| s.contains("int add(int a, int b)")));
}

// -- C++ (reuses C grammar) ------------------------------------------------
#[test]
fn cpp_extract() {
    let src = r#"
struct Point { int x; int y; };
int add(int a, int b) { return a + b; }
"#;
    let idx = run("cpp", src);
    let tops = top_sigs(&idx);
    assert!(tops.iter().any(|s| s.contains("struct Point")));
    assert!(tops.iter().any(|s| s.contains("int add")));
}

// -- JavaScript -------------------------------------------------------------
#[test]
fn javascript_extract() {
    // Note: `export class Foo {}` is handled as an `export_statement` at the
    // top level and doesn't currently recurse into class methods. Use the
    // plain `class` form here to exercise method nesting.
    let src = r#"
function hello(name) {
    return "hi " + name;
}

class Greeter {
    constructor(name) { this.name = name; }
    greet() { return "hi " + this.name; }
}

const VERSION = "1.0";
"#;
    let idx = run("javascript", src);
    let tops = top_sigs(&idx);
    assert!(tops.iter().any(|s| s.contains("function hello")));
    assert!(tops.iter().any(|s| s.contains("class Greeter")));

    let greeter = idx
        .symbols
        .iter()
        .find(|s| s.signature.contains("class Greeter"))
        .unwrap();
    let child_sigs: Vec<&str> = greeter
        .children
        .iter()
        .map(|c| c.signature.as_str())
        .collect();
    assert!(child_sigs.iter().any(|s| s.contains("constructor")));
    assert!(child_sigs.iter().any(|s| s.contains("greet")));
}

// -- TypeScript -------------------------------------------------------------
#[test]
fn typescript_extract() {
    let src = r#"
export interface User {
    name: string;
    age: number;
}

export type Id = string;

export class Account {
    constructor(public user: User) {}
    getName(): string { return this.user.name; }
}

export enum Color { Red, Green, Blue }

export function main(): void {}
"#;
    let idx = run("typescript", src);
    let tops = top_sigs(&idx);
    assert!(tops.iter().any(|s| s.contains("interface User")));
    assert!(tops.iter().any(|s| s.contains("type Id")));
    assert!(tops.iter().any(|s| s.contains("class Account")));
    assert!(tops.iter().any(|s| s.contains("enum Color")));
    assert!(tops.iter().any(|s| s.contains("function main")));
}

// -- C# ---------------------------------------------------------------------
#[test]
fn c_sharp_extract() {
    let src = r#"
namespace App {
    public class User {
        public string Name { get; set; }
        public void Greet() { }
    }

    public interface IRunnable {
        void Run();
    }

    public enum Status { Ok, Err }
}
"#;
    let idx = run("c_sharp", src);
    // The namespace is a container; flatten up to two levels to find members.
    let flat: Vec<&str> = idx
        .symbols
        .iter()
        .flat_map(|s| {
            std::iter::once(s.signature.as_str())
                .chain(s.children.iter().map(|c| c.signature.as_str()))
                .chain(
                    s.children
                        .iter()
                        .flat_map(|c| c.children.iter().map(|cc| cc.signature.as_str())),
                )
        })
        .collect();
    assert!(flat.iter().any(|s| s.contains("class User")));
    assert!(flat.iter().any(|s| s.contains("interface IRunnable")));
    assert!(flat.iter().any(|s| s.contains("enum Status")));
}

// -- Java -------------------------------------------------------------------
#[test]
fn java_extract() {
    let src = r#"
package com.example;

public class User {
    public String name;
    public void greet() { }
}

interface Runnable2 {
    void run();
}

enum Color { RED, GREEN, BLUE }
"#;
    let idx = run("java", src);
    let tops = top_sigs(&idx);
    assert!(tops.iter().any(|s| s.contains("class User")));
    assert!(tops.iter().any(|s| s.contains("interface Runnable2")));
    assert!(tops.iter().any(|s| s.contains("enum Color")));

    let user = idx
        .symbols
        .iter()
        .find(|s| s.signature.contains("class User"))
        .unwrap();
    let child_sigs: Vec<&str> = user.children.iter().map(|c| c.signature.as_str()).collect();
    assert!(child_sigs.iter().any(|s| s.contains("greet")));
}

// -- Ruby -------------------------------------------------------------------
#[test]
fn ruby_extract() {
    let src = r#"
module Greeter
  class Person
    def initialize(name)
      @name = name
    end

    def greet
      "hi"
    end
  end

  def helper
    "h"
  end
end
"#;
    let idx = run("ruby", src);
    let tops = top_sigs(&idx);
    assert!(tops.iter().any(|s| s.starts_with("module Greeter")));

    // Greeter module should contain Person class and `helper` method,
    // but NOT any spurious bare "module" keyword entry (regression for
    // the `is_named()` filter fix).
    let greeter = idx
        .symbols
        .iter()
        .find(|s| s.signature.starts_with("module Greeter"))
        .unwrap();
    let child_sigs: Vec<&str> = greeter
        .children
        .iter()
        .map(|c| c.signature.as_str())
        .collect();
    assert!(child_sigs.iter().any(|s| s.starts_with("class Person")));
    assert!(child_sigs.iter().any(|s| s.contains("def helper")));
    assert!(
        !child_sigs.contains(&"module"),
        "spurious bare `module` keyword leaked into children: {:?}",
        child_sigs
    );

    // Person (nested in module) should carry its own methods.
    let person = greeter
        .children
        .iter()
        .find(|s| s.signature.starts_with("class Person"))
        .expect("Person nested class missing");
    let person_methods: Vec<&str> = person
        .children
        .iter()
        .map(|c| c.signature.as_str())
        .collect();
    assert!(person_methods.iter().any(|s| s.contains("initialize")));
    assert!(person_methods.iter().any(|s| s.contains("def greet")));
}

// -- PHP --------------------------------------------------------------------
#[test]
fn php_extract() {
    let src = r#"<?php
namespace App;

function helper(): string { return "h"; }

class User {
    public function getName(): string { return $this->name; }
}

interface Printable {
    public function toString(): string;
}

trait Loggable {
    public function log(string $m): void { echo $m; }
}

enum Color {
    case Red;
    case Green;
}
"#;
    let idx = run("php", src);
    let tops = top_sigs(&idx);
    assert!(tops.iter().any(|s| s.contains("function helper")));
    assert!(tops.iter().any(|s| s.contains("class User")));
    assert!(tops.iter().any(|s| s.contains("interface Printable")));
    assert!(tops.iter().any(|s| s.contains("trait Loggable")));
    assert!(tops.iter().any(|s| s.contains("enum Color")));

    let color = idx
        .symbols
        .iter()
        .find(|s| s.signature.contains("enum Color"))
        .unwrap();
    let variants: Vec<&str> = color
        .children
        .iter()
        .map(|c| c.signature.as_str())
        .collect();
    assert!(variants.iter().any(|s| s.contains("Red")));
    assert!(variants.iter().any(|s| s.contains("Green")));
}

// -- Lua --------------------------------------------------------------------
#[test]
fn lua_extract() {
    let src = r#"
function greet(name)
    return "hi " .. name
end

local function helper()
    return 42
end

local config = { debug = true }
"#;
    let idx = run("lua", src);
    let tops = top_sigs(&idx);
    assert!(tops.iter().any(|s| s.contains("function greet")));
    assert!(tops.iter().any(|s| s.contains("helper")));
}

// -- Zig --------------------------------------------------------------------
#[test]
fn zig_extract() {
    let src = r#"
const std = @import("std");

pub fn add(a: i32, b: i32) i32 {
    return a + b;
}

fn privateHelper() void {}

const MAX: usize = 1024;

test "addition" {
    _ = add(2, 3);
}
"#;
    let idx = run("zig", src);
    let tops = top_sigs(&idx);
    assert!(tops.iter().any(|s| s.contains("pub fn add")));
    assert!(tops.iter().any(|s| s.contains("fn privateHelper")));
    assert!(tops.iter().any(|s| s.contains("MAX")));
    assert!(tops.iter().any(|s| s.contains("test \"addition\"")));
}

// -- Swift ------------------------------------------------------------------
#[test]
fn swift_extract() {
    let src = r#"
protocol Drawable {
    func draw()
}

struct Point {
    var x: Double
    var y: Double

    init(x: Double, y: Double) {
        self.x = x
        self.y = y
    }

    func magnitude() -> Double { return 0 }
}

class Shape {
    var name: String
    init(name: String) { self.name = name }
}

enum Direction {
    case north
    case south
}

extension Point {
    func translated() -> Point { return self }
}

typealias Coord = Point

func globalFn() -> Int { return 1 }
"#;
    let idx = run("swift", src);
    let tops = top_sigs(&idx);
    assert!(tops.iter().any(|s| s.contains("protocol Drawable")));
    assert!(tops.iter().any(|s| s.contains("struct Point")));
    assert!(tops.iter().any(|s| s.contains("class Shape")));
    assert!(tops.iter().any(|s| s.contains("enum Direction")));
    assert!(tops.iter().any(|s| s.contains("extension Point")));
    assert!(tops.iter().any(|s| s.contains("typealias Coord")));
    assert!(tops.iter().any(|s| s.contains("func globalFn")));

    let point = idx
        .symbols
        .iter()
        .find(|s| s.signature.contains("struct Point"))
        .unwrap();
    let members: Vec<&str> = point
        .children
        .iter()
        .map(|c| c.signature.as_str())
        .collect();
    assert!(members.iter().any(|s| s.contains("magnitude")));
}

// -- Scala ------------------------------------------------------------------
#[test]
fn scala_extract() {
    let src = r#"
package com.example

trait Greeter {
  def greet(name: String): String
}

class Person(val name: String) {
  def introduce(): String = s"I'm $name"
  val greeting: String = "hi"

  class Inner {
    def help(): Unit = ()
  }
}

object Main {
  def main(args: Array[String]): Unit = println("hi")
}

type Id = String
"#;
    let idx = run("scala", src);
    let tops = top_sigs(&idx);
    assert!(tops.iter().any(|s| s.contains("trait Greeter")));
    assert!(tops.iter().any(|s| s.contains("class Person")));
    assert!(tops.iter().any(|s| s.contains("object Main")));
    assert!(tops.iter().any(|s| s.contains("type Id")));

    // Person should have its methods AND its nested Inner class, with Inner
    // carrying its own method (multi-level nesting — regression for the
    // `collect_nested` recursion fix).
    let person = idx
        .symbols
        .iter()
        .find(|s| s.signature.contains("class Person"))
        .unwrap();
    let p_members: Vec<&str> = person
        .children
        .iter()
        .map(|c| c.signature.as_str())
        .collect();
    assert!(p_members.iter().any(|s| s.contains("introduce")));
    assert!(p_members.iter().any(|s| s.contains("greeting")));

    let inner = person
        .children
        .iter()
        .find(|s| s.signature.contains("class Inner"))
        .expect("Inner class missing");
    let inner_methods: Vec<&str> = inner
        .children
        .iter()
        .map(|c| c.signature.as_str())
        .collect();
    assert!(inner_methods.iter().any(|s| s.contains("help")));
}

// -- Shell ------------------------------------------------------------------
#[test]
fn shell_extract() {
    let src = r#"#!/usr/bin/env bash

greet() {
    echo "hi $1"
}

function other {
    echo "other"
}
"#;
    let idx = run("shell", src);
    let tops = top_sigs(&idx);
    assert!(tops.iter().any(|s| s.contains("greet")));
    assert!(tops.iter().any(|s| s.contains("other")));
}

// -- Markdown ---------------------------------------------------------------
#[test]
fn markdown_extract() {
    let src = r#"# Top

Intro.

## Section One

Text.

### Sub

More.

## Section Two
"#;
    let idx = run("markdown", src);
    assert_eq!(idx.symbols.len(), 1);
    let top = &idx.symbols[0];
    assert_eq!(top.signature, "# Top");
    assert!(matches!(top.kind, SymbolKind::Heading(1)));

    let h2_sigs: Vec<&str> = top.children.iter().map(|c| c.signature.as_str()).collect();
    assert!(h2_sigs.contains(&"## Section One"));
    assert!(h2_sigs.contains(&"## Section Two"));

    let sec1 = top
        .children
        .iter()
        .find(|c| c.signature == "## Section One")
        .unwrap();
    assert!(sec1.children.iter().any(|c| c.signature == "### Sub"));
}

// -- Cross-cutting: nested items retain grandchildren -----------------------
#[test]
fn rust_impl_methods_nested() {
    // Regression: before the `collect_nested` recursion fix, nested symbols
    // found via `find_in_subtree` got empty children vectors.
    let src = r#"
pub struct S;
impl S {
    pub fn a(&self) {}
    pub fn b(&self) {}
}
"#;
    let idx = run("rust", src);
    let imp = idx
        .symbols
        .iter()
        .find(|s| s.signature.starts_with("impl S"))
        .unwrap();
    let methods: Vec<&str> = imp.children.iter().map(|c| c.signature.as_str()).collect();
    assert!(methods.iter().any(|s| s.contains("fn a")));
    assert!(methods.iter().any(|s| s.contains("fn b")));
}

// -- Unknown language returns an error -------------------------------------
#[test]
fn unknown_language_errors() {
    let result = extract_from_bytes(Path::new("fixture"), "brainfuck", b"+++");
    assert!(result.is_err());
}

// -- Line numbers are 1-indexed --------------------------------------------
#[test]
fn line_numbers_one_indexed() {
    let src = "fn main() {}\n";
    let idx = run("rust", src);
    assert_eq!(idx.symbols[0].line_start, 1);
}
