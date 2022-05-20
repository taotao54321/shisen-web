use std::num::NonZeroUsize;

use itertools::{Either, Itertools as _};
use rand::prelude::*;

use crate::util;

/// 牌の種類数。
pub const TILE_KIND_COUNT: usize = 34;

/// 盤面上のマス。
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Square {
    pub c: usize,
    pub r: usize,
}

impl Square {
    pub fn new(c: usize, r: usize) -> Self {
        Self { c, r }
    }
}

/// 盤面上のマスの中身。
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BoardCell {
    Empty,
    Tile(usize),
}

impl BoardCell {
    pub fn is_empty(self) -> bool {
        matches!(self, Self::Empty)
    }

    pub fn is_tile(self) -> bool {
        matches!(self, Self::Tile(_))
    }

    pub fn is_same_tile(self, other: BoardCell) -> bool {
        match (self, other) {
            (Self::Tile(kind1), Self::Tile(kind2)) => kind1 == kind2,
            _ => false,
        }
    }
}

/// 盤面。
#[derive(Clone, Debug)]
pub struct Board {
    ncol: NonZeroUsize,
    nrow: NonZeroUsize,
    cells: Vec<BoardCell>,
}

impl Board {
    /// 空の盤面を返す。
    ///
    /// `ncol_inner`, `nrow_inner` は外周を除くサイズ。
    /// 少なくとも一方は偶数でなければならない。
    ///
    /// `ncol_inner * nrow_inner` がオーバーフローする場合、panic する。
    pub fn empty(ncol_inner: NonZeroUsize, nrow_inner: NonZeroUsize) -> Self {
        assert!(ncol_inner.get() % 2 == 0 || nrow_inner.get() % 2 == 0);

        let ncol =
            NonZeroUsize::new(ncol_inner.get().checked_add(2).expect("ncol overflow")).unwrap();
        let nrow =
            NonZeroUsize::new(nrow_inner.get().checked_add(2).expect("nrow overflow")).unwrap();

        let n = ncol.get().checked_mul(nrow.get()).expect("n overflow");

        let cells = vec![BoardCell::Empty; n];

        Self { ncol, nrow, cells }
    }

    /// ランダムな盤面を返す。解の存在が保証される。
    ///
    /// `ncol_inner`, `nrow_inner` は外周を除くサイズ。
    /// 少なくとも一方は偶数でなければならない。
    pub fn random(ncol_inner: NonZeroUsize, nrow_inner: NonZeroUsize) -> Self {
        let mut this = Self::empty(ncol_inner, nrow_inner);

        // 全種類の牌をなるべく均等に出現させる。
        // 端数の分はランダムに割り振る。

        let n_inner = ncol_inner.get() * nrow_inner.get();
        let q = n_inner / (2 * TILE_KIND_COUNT);
        let r = n_inner % (2 * TILE_KIND_COUNT);

        let mut tiles = Vec::<usize>::with_capacity(n_inner);
        {
            let mut xs: Vec<_> = (0..TILE_KIND_COUNT).collect();
            for _ in 0..2 * q {
                tiles.extend(xs.iter());
            }
            xs.shuffle(&mut thread_rng());
            for _ in 0..2 {
                tiles.extend(xs[..r / 2].iter());
            }
        }

        for (sq, tile) in itertools::zip_eq(this.squares_inner(), tiles) {
            this[sq] = BoardCell::Tile(tile);
        }

        this.shuffle_solvable();

        this
    }

    /// 列数を返す。
    pub fn ncol(&self) -> NonZeroUsize {
        self.ncol
    }

    /// 行数を返す。
    pub fn nrow(&self) -> NonZeroUsize {
        self.nrow
    }

    /// 盤面が空かどうかを返す。
    pub fn is_empty(&self) -> bool {
        self.squares_inner().all(|sq| self[sq].is_empty())
    }

    /// 盤面が空でなく、かつ手詰まり状態かどうかを返す。
    pub fn is_stuck(&self) -> bool {
        !self.is_empty() && self.find_move().is_none()
    }

    /// 盤面上の全マスを列挙する。外周も含む。
    pub fn squares(&self) -> impl Iterator<Item = Square> {
        let ncol = self.ncol.get();
        let nrow = self.nrow.get();

        itertools::iproduct!(0..nrow, 0..ncol).map(|(r, c)| Square::new(c, r))
    }

    /// 盤面上の外周を除いた全マスを列挙する。
    pub fn squares_inner(&self) -> impl Iterator<Item = Square> {
        let ncol = self.ncol.get();
        let nrow = self.nrow.get();

        self.squares().filter(move |&Square { c, r }| {
            (1..ncol - 1).contains(&c) && (1..nrow - 1).contains(&r)
        })
    }

    /// 盤面上の全ての牌を列挙する。
    pub fn iter_tiles(&self) -> impl Iterator<Item = BoardCell> + '_ {
        self.enumerate_tiles().map(|e| e.1)
    }

    /// 盤面上の全ての牌をマス付きで列挙する。
    pub fn enumerate_tiles(&self) -> impl Iterator<Item = (Square, BoardCell)> + '_ {
        self.squares_inner().filter_map(|sq| {
            if let x @ BoardCell::Tile(_) = self[sq] {
                Some((sq, x))
            } else {
                None
            }
        })
    }

    /// 着手を行う。`mv` は合法と仮定している。
    pub fn do_move(&mut self, mv: &Move) {
        self[mv.src()] = BoardCell::Empty;
        self[mv.dst()] = BoardCell::Empty;
    }

    /// 盤面上の全ての牌について、位置を変えずにシャッフルする。
    /// 結果の盤面は解を持つことが保証される。
    pub fn shuffle_solvable(&mut self) {
        // シャッフルしてから合法手がなくなるまでランダムな着手を続ける。
        // これを盤面が空になるまで繰り返す。

        // 作業はコピーした盤面上で行い、シャッフル結果を self に書き戻す。
        let mut board = self.clone();

        while !board.is_empty() {
            board.shuffle();

            for (sq, tile) in board.enumerate_tiles() {
                self[sq] = tile;
            }

            while let Some(mv) = board.random_move() {
                board.do_move(&mv);
            }
        }
    }

    /// 盤面上の全ての牌について、位置を変えずにシャッフルする。
    /// 結果の盤面は解を持つとは限らない。
    fn shuffle(&mut self) {
        let mut tiles: Vec<_> = self.iter_tiles().collect();
        tiles.shuffle(&mut thread_rng());

        // 逆順になるが、どうせシャッフルしてるので問題ない。
        for sq in self.squares_inner() {
            if self[sq].is_tile() {
                self[sq] = tiles.pop().expect("tiles should be nonempty");
            }
        }
    }

    /// 現在の盤面における合法手を 0 または 1 個返す。単純な全探索による。
    pub fn find_move(&self) -> Option<Move> {
        self.squares_inner()
            .combinations(2)
            .flat_map(|sqs| self.find_move_between(sqs[0], sqs[1]))
            .next()
    }

    /// 現在の盤面におけるランダムな合法手を 0 または 1 個返す。
    pub fn random_move(&self) -> Option<Move> {
        let mut combs: Vec<_> = self.squares_inner().combinations(2).collect();
        combs.shuffle(&mut thread_rng());

        combs
            .into_iter()
            .flat_map(|sqs| self.find_move_between(sqs[0], sqs[1]))
            .next()
    }

    /// 指定した 2 マスに対する合法手を 0 または 1 個返す。
    pub fn find_move_between(&self, src: Square, dst: Square) -> Option<Move> {
        self.moves_between(src, dst).next()
    }

    /// 指定した 2 マスに対する最短経路の合法手を 0 または 1 個返す。
    pub fn shortest_move_between(&self, src: Square, dst: Square) -> Option<Move> {
        self.moves_between(src, dst)
            .min_by_key(|mv| mv.path_distance())
    }

    /// 指定した 2 マスに対する合法手(全ての経路)を列挙する。
    fn moves_between(&self, src: Square, dst: Square) -> impl Iterator<Item = Move> + '_ {
        // src, dst が同一なら違法。
        // src, dst の牌種が異なるなら違法。
        if src == dst || !self[src].is_same_tile(self[dst]) {
            return Either::Left(std::iter::empty());
        }

        // 二角取りの経路は、縦-横-縦 または 横-縦-横 のいずれか。
        Either::Right(
            self.moves_between_vhv(src, dst)
                .chain(self.moves_between_hvh(src, dst)),
        )
    }

    /// 指定した 2 マスに対する 縦-横-縦 の全ての経路を列挙する。
    fn moves_between_vhv(&self, src: Square, dst: Square) -> impl Iterator<Item = Move> + '_ {
        // src, dst の列が同じなら 縦-横-縦 の経路はない。
        if src.c == dst.c {
            return Either::Left(std::iter::empty());
        }

        let r_range = {
            let f_min = |sq: Square| {
                (0..sq.r)
                    .rev()
                    .find(|&r| self[Square::new(sq.c, r)].is_tile())
                    .map(|r| r + 1)
                    .unwrap_or(0)
            };
            let f_max = |sq: Square| {
                (sq.r + 1..self.nrow.get())
                    .find(|&r| self[Square::new(sq.c, r)].is_tile())
                    .map(|r| r - 1)
                    .unwrap_or(self.nrow.get() - 1)
            };
            let range_src = f_min(src)..=f_max(src);
            let range_dst = f_min(dst)..=f_max(dst);
            util::range_intersection(range_src, range_dst)
        };

        let c_range = {
            let min = src.c.min(dst.c) + 1;
            let max = src.c.max(dst.c) - 1;
            min..=max
        };

        Either::Right(
            r_range
                .filter(move |&r| c_range.clone().all(|c| self[Square::new(c, r)].is_empty()))
                .map(move |r| Move::new_vhv(src, dst, r)),
        )
    }

    /// 指定した 2 マスに対する 横-縦-横 の経路を列挙する。
    fn moves_between_hvh(&self, src: Square, dst: Square) -> impl Iterator<Item = Move> + '_ {
        // src, dst の行が同じなら 横-縦-横 の経路はない。
        if src.r == dst.r {
            return Either::Left(std::iter::empty());
        }

        let c_range = {
            let f_min = |sq: Square| {
                (0..sq.c)
                    .rev()
                    .find(|&c| self[Square::new(c, sq.r)].is_tile())
                    .map(|c| c + 1)
                    .unwrap_or(0)
            };
            let f_max = |sq: Square| {
                (sq.c + 1..self.ncol.get())
                    .find(|&c| self[Square::new(c, sq.r)].is_tile())
                    .map(|c| c - 1)
                    .unwrap_or(self.ncol.get() - 1)
            };
            let range_src = f_min(src)..=f_max(src);
            let range_dst = f_min(dst)..=f_max(dst);
            util::range_intersection(range_src, range_dst)
        };

        let r_range = {
            let min = src.r.min(dst.r) + 1;
            let max = src.r.max(dst.r) - 1;
            min..=max
        };

        Either::Right(
            c_range
                .filter(move |&c| r_range.clone().all(|r| self[Square::new(c, r)].is_empty()))
                .map(move |c| Move::new_hvh(src, dst, c)),
        )
    }

    fn cr2idx(&self, c: usize, r: usize) -> usize {
        self.ncol.get() * r + c
    }

    fn sq2idx(&self, sq: Square) -> usize {
        self.cr2idx(sq.c, sq.r)
    }
}

impl std::ops::Index<Square> for Board {
    type Output = BoardCell;

    fn index(&self, sq: Square) -> &Self::Output {
        let idx = self.sq2idx(sq);
        &self.cells[idx]
    }
}

impl std::ops::IndexMut<Square> for Board {
    fn index_mut(&mut self, sq: Square) -> &mut Self::Output {
        let idx = self.sq2idx(sq);
        &mut self.cells[idx]
    }
}

/// 着手。
#[derive(Debug)]
pub struct Move {
    path: Vec<Square>,
}

impl Move {
    /// 縦-横-縦 の経路による着手を作る。
    fn new_vhv(src: Square, dst: Square, r: usize) -> Self {
        assert_ne!(src.c, dst.c);

        let mut path = Vec::<Square>::with_capacity(4);

        path.push(src);

        if src.r != r {
            path.push(Square::new(src.c, r));
        }

        if dst.r != r {
            path.push(Square::new(dst.c, r));
        }

        path.push(dst);

        Self { path }
    }

    /// 横-縦-横 の経路による着手を作る。
    fn new_hvh(src: Square, dst: Square, c: usize) -> Self {
        assert_ne!(src.r, dst.r);

        let mut path = Vec::<Square>::with_capacity(4);

        path.push(src);

        if src.c != c {
            path.push(Square::new(c, src.r));
        }

        if dst.c != c {
            path.push(Square::new(c, dst.r));
        }

        path.push(dst);

        Self { path }
    }

    /// 始点を返す。
    pub fn src(&self) -> Square {
        *self.path.first().expect("path should be nonempty")
    }

    /// 終点を返す。
    pub fn dst(&self) -> Square {
        *self.path.last().expect("path should be nonempty")
    }

    /// 経路を返す。
    pub fn path(&self) -> &[Square] {
        &self.path
    }

    /// 経路長を返す。
    fn path_distance(&self) -> usize {
        self.path
            .windows(2)
            .map(|e| <&[Square; 2]>::try_from(e).expect("window length should be 2"))
            .map(|[sq1, sq2]| {
                let dc = sq1.c.abs_diff(sq2.c);
                let dr = sq1.r.abs_diff(sq2.r);
                dc.max(dr)
            })
            .sum()
    }
}
