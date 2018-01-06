use std::sync::Arc;
use std::cmp::max;
use std::iter::FromIterator;
use std::str::FromStr;
use std::ops::Add;
use std::fmt::{Debug, Display, Error, Formatter};

lazy_static! {
    static ref EMPTY: Text = Leaf(Arc::new(String::new()));
}

use self::Text::*;

const LEAF_MAX: usize = 1000;

pub enum Text {
    Branch(Arc<TextNode>),
    Leaf(Arc<String>),
}

#[derive(Clone, PartialEq, Eq)]
pub struct TextNode {
    left: Text,
    right: Text,
    length: usize,
    depth: usize,
    lines: usize,
}

impl Text {
    #[inline]
    pub fn new() -> Self {
        EMPTY.clone()
    }

    fn copy_from_str(r: &str) -> Self {
        let target = match r.chars().position(|c| c == '\n') {
            Some(lf) if lf < (r.len() - 1) => lf + 1,
            _ if r.len() > LEAF_MAX => r.len() / 2,
            _ => return Leaf(Arc::new(r.to_string())),
        };
        let left = r.chars().take(target).collect();
        let right = r.chars().skip(target).collect();
        Leaf(Arc::new(left)).concat(&Leaf(Arc::new(right)))
    }

    pub fn len(&self) -> usize {
        match *self {
            Branch(ref node) => node.length,
            Leaf(ref string) => string.len(),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn lines(&self) -> usize {
        match *self {
            Branch(ref node) => node.lines,
            Leaf(ref string) => string.chars().filter(|c| c == &'\n').count(),
        }
    }

    fn depth(&self) -> usize {
        match *self {
            Branch(ref node) => node.depth,
            Leaf(_) => 0,
        }
    }

    fn is_leaf(&self) -> bool {
        match *self {
            Leaf(_) => true,
            _ => false,
        }
    }

    pub fn char_at(&self, index: usize) -> Option<char> {
        if index >= self.len() {
            None
        } else {
            match *self {
                Leaf(ref string) => string.chars().nth(index),
                Branch(ref node) => {
                    let l = node.left.len();
                    if index < l {
                        node.left.char_at(index)
                    } else {
                        node.right.char_at(index - l)
                    }
                }
            }
        }
    }

    pub fn substr(&self, start: usize, len: usize) -> Self {
        match *self {
            Leaf(ref string) => Leaf(Arc::new(string.chars().skip(start).take(len).collect())),
            Branch(ref node) => {
                let rll = node.left.len();
                let left = if start == 0 && len >= rll {
                    node.left.clone()
                } else {
                    node.left.substr(start, len)
                };
                let ll = left.len();
                let right = if start <= rll && (start + len) >= (rll + node.right.len()) {
                    node.right.clone()
                } else {
                    let split_start = if start > rll { start - rll } else { 0 };
                    let split_len = if len > ll { len - ll } else { 0 };
                    node.right.substr(split_start, split_len)
                };
                left.concat(&right)
            }
        }
    }

    pub fn take_left(&self, count: usize) -> (Self, Self) {
        if count > self.len() {
            (self.clone(), Text::new())
        } else {
            (
                self.substr(0, count),
                self.substr(count, self.len() - count),
            )
        }
    }

    pub fn take_right(&self, count: usize) -> (Self, Self) {
        if count > self.len() {
            (Text::new(), self.clone())
        } else {
            let split = self.len() - count;
            (self.substr(0, split), self.substr(split, count))
        }
    }

    fn reorder_leaf(&self) -> Self {
        match *self {
            Leaf(ref string) => From::from(&**string),
            _ => self.clone(),
        }
    }

    pub fn concat<'a, T: Into<&'a Text>>(&self, other: T) -> Self {
        let left = self.reorder_leaf();
        let right = other.into().reorder_leaf();
        let ll = left.len();
        if ll == 0 {
            return right;
        }
        let rl = right.len();
        if rl == 0 {
            return left;
        }
        let threshold = LEAF_MAX;
        match (&left, &right) {
            (&Leaf(ref ls), &Leaf(ref rs))
                if ll + rl < threshold && left.char_at(ll - 1) != Some('\n') =>
            {
                Leaf(Arc::new(ls.chars().chain(rs.chars()).collect()))
            }
            (&Branch(ref node), &Leaf(ref rs))
                if node.right.is_leaf() && node.right.char_at(node.right.len() - 1) != Some('\n')
                    && node.right.len() + rl < threshold =>
            {
                match node.right {
                    Leaf(ref ls) => node.left
                        .concat(&Leaf(Arc::new(ls.chars().chain(rs.chars()).collect()))),
                    _ => unreachable!(),
                }
            }
            _ => Branch(Arc::new(TextNode {
                left: left.clone(),
                right: right.clone(),
                length: ll + rl,
                depth: max(left.depth(), right.depth()) + 1,
                lines: left.lines() + right.lines(),
            })),
        }
    }

    pub fn insert(&self, index: usize, other: &Text) -> Self {
        self.substr(0, index)
            .concat(other)
            .concat(&self.substr(index, self.len() - index))
    }

    pub fn delete(&self, index: usize, count: usize) -> Self {
        let right = index + count;
        self.substr(0, index)
            .concat(&self.substr(right, self.len() - right))
    }

    // fn rebalance(&self) -> Self {
    //     if self.len() == 0 {
    //         return self.clone()
    //     }
    //     let mut slot: Vec<Option<Text>> = (0..self.depth() + 2).map(|_| None).collect();

    // }

    fn find_line(&self, line: usize, offset: usize) -> Option<usize> {
        if line == 0 {
            return Some(offset);
        }
        if line >= self.lines() {
            return None;
        }
        match *self {
            Leaf(_) => Some(offset),
            Branch(ref node) => {
                let ll = node.left.lines();
                if line < ll {
                    node.left.find_line(line, offset)
                } else {
                    node.right.find_line(line - ll, offset + node.left.len())
                }
            }
        }
    }

    /// Get the offset into the rope where a given line starts.
    pub fn line_pos(&self, line: usize) -> Option<usize> {
        self.find_line(line, 0)
    }

    /// Make a subrope from the start of a given line to the end of the rope.
    pub fn skip_until_line(&self, line: usize) -> Option<Self> {
        self.line_pos(line)
            .map(|pos| self.substr(pos, self.len() - pos))
    }

    /// Get the contents of a given line as a subrope.
    pub fn line(&self, line: usize) -> Option<Self> {
        let start = self.line_pos(line);
        // TODO could write a function which gets both start and end of a line
        let end = self.line_pos(line + 1);
        match (start, end) {
            (None, _) => None,
            (Some(start), None) => Some(self.substr(start, self.len() - start)),
            (Some(start), Some(end)) => Some(self.substr(start, end - start)),
        }
    }

    pub fn iter(&self) -> Iter {
        Iter::new(self)
    }

    pub fn iter_lines(&self) -> LineIter {
        LineIter::new(self)
    }
}

// Core traits

impl Clone for Text {
    fn clone(&self) -> Self {
        match *self {
            Branch(ref node) => Branch(node.clone()),
            Leaf(ref string) => Leaf(string.clone()),
        }
    }
}

impl PartialEq for Text {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        match (self, other) {
            (&Leaf(ref l), &Leaf(ref r)) if Arc::ptr_eq(l, r) => true,
            (&Branch(ref l), &Branch(ref r)) if Arc::ptr_eq(l, r) => true,
            (&Leaf(ref l), &Leaf(ref r)) => l.eq(r),
            (&Branch(ref l), &Branch(ref r)) => l.eq(r),
            _ => false,
        }
    }
}

impl Eq for Text {}

enum IterResult {
    Next(Arc<String>),
    Walk,
    Done,
}

impl Default for Text {
    fn default() -> Self {
        Self::new()
    }
}

impl ToString for Text {
    fn to_string(&self) -> String {
        match *self {
            Leaf(ref string) => (**string).clone(),
            Branch(ref node) => {
                let mut out = String::new();
                out.push_str(&node.left.to_string());
                out.push_str(&node.right.to_string());
                out
            }
        }
    }
}

impl<'a> Into<String> for &'a Text {
    fn into(self) -> String {
        self.to_string()
    }
}

impl Into<String> for Text {
    fn into(self) -> String {
        self.to_string()
    }
}

impl FromStr for Text {
    type Err = (); // Can't fail.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::copy_from_str(s))
    }
}

impl<'a> From<&'a str> for Text {
    fn from(s: &'a str) -> Self {
        Self::copy_from_str(s)
    }
}

impl<'a> From<&'a String> for Text {
    fn from(s: &'a String) -> Self {
        Self::copy_from_str(s)
    }
}

impl From<String> for Text {
    fn from(s: String) -> Self {
        Self::copy_from_str(&s)
    }
}

impl<'a, 'b> Add<&'b Text> for &'a Text {
    type Output = Text;

    fn add(self, other: &'b Text) -> Self::Output {
        self.concat(other)
    }
}

impl<'a> Add<&'a Text> for Text {
    type Output = Text;

    fn add(self, other: &'a Text) -> Self::Output {
        self.concat(other)
    }
}

impl<'a, S: Into<Text>> Add<S> for &'a Text {
    type Output = Text;

    fn add(self, other: S) -> Self::Output {
        self.concat(&other.into())
    }
}

impl<S: Into<Text>> Add<S> for Text {
    type Output = Text;

    fn add(self, other: S) -> Self::Output {
        self.concat(&other.into())
    }
}

impl Display for Text {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        for s in self {
            Display::fmt(&s, f)?;
        }
        Ok(())
    }
}

impl Debug for Text {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "\"")?;
        for s in self {
            Display::fmt(&s, f)?;
        }
        write!(f, "\"")
    }
}

// Iterators

pub struct Iter {
    stack: Vec<Text>,
}

impl Iter {
    fn new(rope: &Text) -> Iter {
        Iter {
            stack: vec![rope.clone()],
        }
    }

    fn step(&mut self) -> IterResult {
        match self.stack.pop() {
            None => IterResult::Done,
            Some(rope) => match rope {
                Leaf(ref string) => IterResult::Next(string.clone()),
                Branch(ref node) => {
                    self.stack.push(node.right.clone());
                    self.stack.push(node.left.clone());
                    IterResult::Walk
                }
            },
        }
    }
}

impl Iterator for Iter {
    type Item = Arc<String>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut action = IterResult::Walk;
        loop {
            match action {
                IterResult::Walk => action = self.step(),
                IterResult::Done => return None,
                IterResult::Next(s) => return Some(s),
            }
        }
    }
}

pub struct LineIter {
    buf: String,
    iter: Iter,
}

impl LineIter {
    fn new(rope: &Text) -> Self {
        LineIter {
            buf: String::new(),
            iter: Iter::new(rope),
        }
    }
}

impl Iterator for LineIter {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter.next() {
                None if !self.buf.is_empty() => return Some(self.buf.drain(..).collect()),
                Some(ref buf) if buf.ends_with('\n') => {
                    self.buf.extend(buf.chars());
                    return Some(self.buf.drain(..).collect());
                }
                Some(ref buf) => self.buf.extend(buf.chars()),
                None => return None,
            }
        }
    }
}

impl<'a> IntoIterator for &'a Text {
    type Item = Arc<String>;
    type IntoIter = Iter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl IntoIterator for Text {
    type Item = Arc<String>;
    type IntoIter = Iter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl FromIterator<String> for Text {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        iter.into_iter()
            .fold(Text::new(), |rope, item| rope.concat(&Leaf(Arc::new(item))))
    }
}

impl FromIterator<char> for Text {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = char>,
    {
        iter.into_iter().fold(Text::new(), |rope, item| {
            rope.concat(&Leaf(Arc::new(vec![item].into_iter().collect())))
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn char_at() {
        let r = Text::from("Hello") + Text::from(" Joe!");
        assert_eq!(Some('!'), r.char_at(9));
    }

    #[test]
    fn concat() {
        let joe = Text::from("Hello") + Text::from(" Joe!\n");
        let mike = Text::from("Hello") + &Text::from(" Mike!\n");
        let robert = Text::from("Hello ") + "Robert!\n";
        let r = joe + mike + robert;
        assert_eq!("Hello Joe!\nHello Mike!\nHello Robert!\n", r.to_string());
    }

    #[test]
    fn substr() {
        let joe = Text::from("Hello") + Text::from(" Joe!\n");
        let mike = Text::from("Hello") + Text::from(" Mike!\n");
        let robert = Text::from("Hello ") + Text::from("Robert!\n");
        let r = joe + mike + robert;
        assert_eq!("o Mike!\nHe", r.substr(15, 10).to_string());
    }

    #[test]
    fn lines() {
        let joe = Text::from("Hello") + Text::from(" Joe!\n");
        let mike = Text::from("Hello") + Text::from(" Mike!\n");
        let robert = Text::from("Hello ") + Text::from("Robert!\n");
        let r = joe + mike + robert;
        assert_eq!(
            "Hello Joe!\nHello Mike!\nHello Robert!\n",
            r.skip_until_line(0).unwrap().to_string()
        );
        assert_eq!(
            "Hello Mike!\nHello Robert!\n",
            r.skip_until_line(1).unwrap().to_string()
        );
        assert_eq!("Hello Robert!\n", r.skip_until_line(2).unwrap().to_string());
        assert_eq!("Hello Joe!\n", r.line(0).unwrap().to_string());
        assert_eq!("Hello Mike!\n", r.line(1).unwrap().to_string());
        assert_eq!("Hello Robert!\n", r.line(2).unwrap().to_string());
    }

    #[test]
    fn iterators() {
        let r = Text::from("Hello Joe!\nHello Mike!\nHello Robert!\nHello Bjarne!\n");
        let mut it = r.iter_lines();
        assert_eq!("Hello Joe!\n", it.next().unwrap());
        assert_eq!("Hello Mike!\n", it.next().unwrap());
        assert_eq!("Hello Robert!\n", it.next().unwrap());
        assert_eq!("Hello Bjarne!\n", it.next().unwrap());
        assert_eq!(None, it.next());
    }
}