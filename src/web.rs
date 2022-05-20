// XXX: el_ref() を使う際は原則 el_key() も併用する。
// DOM 構造が変わらない場合に el_ref() が効かない問題を避けるため。
// (seed の差分更新の問題?同じ DOM 要素に対して異なる ElRef インスタンスを生成すると壊れるのかも)

use std::num::NonZeroUsize;
use std::time::Duration;

use instant::Instant;
use seed::{prelude::*, *};
use web_sys::{HtmlCanvasElement, MouseEvent};

use crate::asset::Asset;
use crate::shisen::{Board, BoardCell, Move, Square};
use crate::util;

const NCOL_INNER: usize = 8;
const NROW_INNER: usize = 7;

const CANVAS_WIDTH: u32 = 450;

const TILE_WIDTH: u32 = CANVAS_WIDTH / (NCOL_INNER + 2) as u32;
const TILE_HEIGHT: u32 = TILE_WIDTH * 4 / 3;

const CANVAS_HEIGHT: u32 = TILE_HEIGHT * (NROW_INNER + 2) as u32;

#[wasm_bindgen(start)]
pub fn start() {
    App::start("app", init, update, view);
}

fn init(_: Url, orders: &mut impl Orders<Msg>) -> Model {
    orders
        .perform_cmd(async {
            Asset::load()
                .await
                .map(Msg::AssetLoad)
                .expect("cannot load asset")
        })
        .stream(streams::interval(16, || Msg::Timer));

    Model::new()
}

fn update(msg: Msg, model: &mut Model, orders: &mut impl Orders<Msg>) {
    let taken = std::mem::take(model);
    *model = taken.update(msg, orders);
}

fn view(model: &Model) -> Node<Msg> {
    model.view()
}

#[derive(Debug)]
enum Msg {
    AssetLoad(Asset),
    Restart,
    ModelInit,
    Timer,
    DrawCanvas,
    CanvasClick(MouseEvent),
}

#[derive(Debug)]
enum Model {
    Loading(ModelLoading),
    Playing(ModelPlaying),
    Win(ModelWin),
    Stuck(ModelStuck),
}

impl Model {
    fn new() -> Self {
        Self::default()
    }

    fn update(self, msg: Msg, orders: &mut impl Orders<Msg>) -> Model {
        match self {
            Model::Loading(inner) => inner.update(msg, orders),
            Model::Playing(inner) => inner.update(msg, orders),
            Model::Win(inner) => inner.update(msg, orders),
            Model::Stuck(inner) => inner.update(msg, orders),
        }
    }

    fn view(&self) -> Node<Msg> {
        match self {
            Model::Loading(inner) => inner.view(),
            Model::Playing(inner) => inner.view(),
            Model::Win(inner) => inner.view(),
            Model::Stuck(inner) => inner.view(),
        }
    }
}

impl Default for Model {
    fn default() -> Self {
        Self::Loading(Default::default())
    }
}

#[derive(Debug, Default)]
struct ModelLoading {}

impl ModelLoading {
    fn update(self, msg: Msg, orders: &mut impl Orders<Msg>) -> Model {
        match msg {
            Msg::AssetLoad(asset) => {
                orders.after_next_render(|_| Msg::ModelInit);
                return Model::Playing(ModelPlaying::new(asset));
            }
            Msg::Timer => {}
            _ => panic!("unexpected message: {msg:?}"),
        }

        Model::Loading(self)
    }

    fn view(&self) -> Node<Msg> {
        div!["loading..."]
    }
}

#[derive(Debug)]
struct ModelPlaying {
    asset: Asset,
    board: Board,
    clock: Instant,
    sq_select: Option<Square>,
    mv_last: Option<Move>,
    path_timer: u32,
    el_canvas: ElRef<HtmlCanvasElement>,
}

impl ModelPlaying {
    fn new(asset: Asset) -> Self {
        let board = Board::random(
            NonZeroUsize::new(NCOL_INNER).unwrap(),
            NonZeroUsize::new(NROW_INNER).unwrap(),
        );

        let clock = Instant::now();

        Self {
            asset,
            board,
            clock,
            sq_select: None,
            mv_last: None,
            path_timer: 0,
            el_canvas: Default::default(),
        }
    }

    fn restart(&mut self) {
        self.board = Board::random(
            NonZeroUsize::new(NCOL_INNER).unwrap(),
            NonZeroUsize::new(NROW_INNER).unwrap(),
        );

        self.clock = Instant::now();

        self.sq_select = None;
        self.mv_last = None;
        self.path_timer = 0;
    }

    fn update(mut self, msg: Msg, orders: &mut impl Orders<Msg>) -> Model {
        match msg {
            Msg::Restart => {
                orders.after_next_render(|_| Msg::ModelInit);
                // XXX: 新しい Model::Playing を返すと el_ref() が効かない問題が起こるので...
                self.restart();
            }
            Msg::ModelInit => {
                orders.after_next_render(|_| Msg::DrawCanvas);
            }
            Msg::Timer => {
                if self.path_timer > 0 {
                    self.path_timer -= 1;
                    if self.path_timer == 0 {
                        orders.after_next_render(|_| Msg::DrawCanvas);
                    }
                }
            }
            Msg::DrawCanvas => {
                self.draw_canvas();
            }
            Msg::CanvasClick(mouse) => {
                if let Some(sq) = self.mouse_pos_to_square(mouse.offset_x(), mouse.offset_y()) {
                    if let Some(sq_select) = self.sq_select {
                        if let Some(mv) = self.board.shortest_move_between(sq_select, sq) {
                            let _ = self.asset.sound_pick().play().unwrap();
                            self.board.do_move(&mv);
                            self.mv_last = Some(mv);
                            self.path_timer = 30;

                            /*
                            // クリアか stuck まで進めてみるテスト
                            while let Some(mv) = self.board.find_move() {
                                self.board.do_move(&mv);
                            }
                            */

                            // クリア判定。
                            if self.board.is_empty() {
                                orders.after_next_render(|_| Msg::ModelInit);
                                return Model::Win(ModelWin::new(self.asset, self.clock.elapsed()));
                            }

                            // stuck 判定。
                            if self.board.is_stuck() {
                                orders.after_next_render(|_| Msg::ModelInit);
                                return Model::Stuck(ModelStuck::new(
                                    self.asset,
                                    self.board,
                                    self.clock.elapsed(),
                                ));
                            }
                        }
                        self.sq_select = None;
                    } else if self.board[sq].is_tile() {
                        self.sq_select = Some(sq);
                    }
                    orders.after_next_render(|_| Msg::DrawCanvas);
                }
            }
            _ => panic!("unexpected message: {msg:?}"),
        }

        Model::Playing(self)
    }

    fn draw_canvas(&self) {
        let canvas = self.el_canvas.get().unwrap();
        let ctx = canvas_context_2d(&canvas);

        // 背景を描画。
        ctx.set_fill_style(&JsValue::from("rgb(0, 128, 64)"));
        ctx.fill_rect(
            0.0,
            0.0,
            f64::from(canvas.width()),
            f64::from(canvas.height()),
        );

        // 牌を描画。
        for sq in self.board.squares_inner() {
            if let BoardCell::Tile(tile) = self.board[sq] {
                let img = self.asset.image_tile(tile);
                // 外周に 1px のマージンを設ける。
                let w = f64::from(TILE_WIDTH);
                let h = f64::from(TILE_HEIGHT);
                let x = 1.0 + w * f64::from(u32::try_from(sq.c).unwrap());
                let y = 1.0 + h * f64::from(u32::try_from(sq.r).unwrap());
                ctx.draw_image_with_image_bitmap_and_dw_and_dh(img, x, y, w - 2.0, h - 2.0)
                    .unwrap();

                // 選択中の牌は強調表示。
                if self.sq_select.map_or(false, |sq_select| sq_select == sq) {
                    ctx.set_fill_style(&JsValue::from("rgba(255, 255, 0, 0.3)"));
                    ctx.fill_rect(x, y, w - 2.0, h - 2.0);
                }
            }
        }

        // 最終手の経路を描画。
        if self.path_timer > 0 {
            ctx.set_line_width(8.0);
            ctx.set_line_cap("round");
            ctx.set_stroke_style(&JsValue::from("orange"));
            ctx.begin_path();
            let mv = self.mv_last.as_ref().expect("mv_last should be some");
            for sqs in mv.path().windows(2) {
                let (x1, y1) = Self::center_of_square(sqs[0]);
                let (x2, y2) = Self::center_of_square(sqs[1]);
                ctx.move_to(x1, y1);
                ctx.line_to(x2, y2);
            }
            ctx.stroke();
        }
    }

    fn center_of_square(sq: Square) -> (f64, f64) {
        let c = f64::from(u32::try_from(sq.c).unwrap());
        let r = f64::from(u32::try_from(sq.r).unwrap());

        let w = f64::from(TILE_WIDTH);
        let h = f64::from(TILE_HEIGHT);

        let x = w * c + w / 2.0;
        let y = h * r + h / 2.0;

        (x, y)
    }

    fn mouse_pos_to_square(&self, x: i32, y: i32) -> Option<Square> {
        // x または y が負なら None を返す。
        let x = match u32::try_from(x) {
            Ok(x) => x,
            Err(_) => return None,
        };
        let y = match u32::try_from(y) {
            Ok(y) => y,
            Err(_) => return None,
        };

        let ncol = self.board.ncol().get();
        let nrow = self.board.nrow().get();

        let c = usize::try_from(x / TILE_WIDTH).unwrap();
        let r = usize::try_from(y / TILE_HEIGHT).unwrap();

        if c >= ncol || r >= nrow {
            return None;
        }

        Some(Square::new(c, r))
    }

    fn view(&self) -> Node<Msg> {
        div![self.view_canvas(), self.view_ui()]
    }

    fn view_canvas(&self) -> Node<Msg> {
        div![canvas![
            el_ref(&self.el_canvas),
            el_key(&"playing_canvas"),
            attrs! {
                At::Width => px(CANVAS_WIDTH),
                At::Height => px(CANVAS_HEIGHT),
            },
            mouse_ev(Ev::Click, Msg::CanvasClick),
        ]]
    }

    fn view_ui(&self) -> Node<Msg> {
        div![
            div![util::format_duration(self.clock.elapsed())],
            div![button!["Restart", ev(Ev::Click, |_| Msg::Restart)]],
        ]
    }
}

#[derive(Debug)]
struct ModelWin {
    asset: Asset,
    elapsed: Duration,
    el_canvas: ElRef<HtmlCanvasElement>,
}

impl ModelWin {
    fn new(asset: Asset, elapsed: Duration) -> Self {
        Self {
            asset,
            elapsed,
            el_canvas: Default::default(),
        }
    }

    fn update(self, msg: Msg, orders: &mut impl Orders<Msg>) -> Model {
        match msg {
            Msg::Restart => {
                orders.after_next_render(|_| Msg::ModelInit);
                return Model::Playing(ModelPlaying::new(self.asset));
            }
            Msg::ModelInit => {
                orders.after_next_render(|_| Msg::DrawCanvas);
            }
            Msg::DrawCanvas => {
                self.draw_canvas();
            }
            Msg::Timer => {}
            _ => panic!("unexpected message: {msg:?}"),
        }

        Model::Win(self)
    }

    fn draw_canvas(&self) {
        let canvas = self.el_canvas.get().unwrap();
        let ctx = canvas_context_2d(&canvas);

        // 背景を描画。
        ctx.set_fill_style(&JsValue::from("rgb(0, 128, 64)"));
        ctx.fill_rect(
            0.0,
            0.0,
            f64::from(canvas.width()),
            f64::from(canvas.height()),
        );
    }

    fn view(&self) -> Node<Msg> {
        div![self.view_canvas(), self.view_ui()]
    }

    fn view_canvas(&self) -> Node<Msg> {
        div![canvas![
            el_ref(&self.el_canvas),
            el_key(&"win_canvas"),
            attrs! {
                At::Width => px(CANVAS_WIDTH),
                At::Height => px(CANVAS_HEIGHT),
            },
        ]]
    }

    fn view_ui(&self) -> Node<Msg> {
        div![
            div![strong![util::format_duration(self.elapsed)]],
            div!["CLEAR!"],
            div![button!["Restart", ev(Ev::Click, |_| Msg::Restart)]],
        ]
    }
}

#[derive(Debug)]
struct ModelStuck {
    asset: Asset,
    board: Board,
    elapsed: Duration,
    el_canvas: ElRef<HtmlCanvasElement>,
}

impl ModelStuck {
    fn new(asset: Asset, board: Board, elapsed: Duration) -> Self {
        Self {
            asset,
            board,
            elapsed,
            el_canvas: Default::default(),
        }
    }

    fn update(self, msg: Msg, orders: &mut impl Orders<Msg>) -> Model {
        match msg {
            Msg::Restart => {
                orders.after_next_render(|_| Msg::ModelInit);
                return Model::Playing(ModelPlaying::new(self.asset));
            }
            Msg::ModelInit => {
                orders.after_next_render(|_| Msg::DrawCanvas);
            }
            Msg::DrawCanvas => {
                self.draw_canvas();
            }
            Msg::Timer => {}
            _ => panic!("unexpected message: {msg:?}"),
        }

        Model::Stuck(self)
    }

    fn draw_canvas(&self) {
        let canvas = self.el_canvas.get().unwrap();
        let ctx = canvas_context_2d(&canvas);

        // 背景を描画。
        ctx.set_fill_style(&JsValue::from("rgb(0, 128, 64)"));
        ctx.fill_rect(
            0.0,
            0.0,
            f64::from(canvas.width()),
            f64::from(canvas.height()),
        );

        // 牌を描画。
        for sq in self.board.squares_inner() {
            if let BoardCell::Tile(tile) = self.board[sq] {
                let img = self.asset.image_tile(tile);
                // 外周に 1px のマージンを設ける。
                let w = f64::from(TILE_WIDTH);
                let h = f64::from(TILE_HEIGHT);
                let x = 1.0 + w * f64::from(u32::try_from(sq.c).unwrap());
                let y = 1.0 + h * f64::from(u32::try_from(sq.r).unwrap());
                ctx.draw_image_with_image_bitmap_and_dw_and_dh(img, x, y, w - 2.0, h - 2.0)
                    .unwrap();
            }
        }

        // 全体を暗くする。
        ctx.set_fill_style(&JsValue::from("rgba(0, 0, 0, 0.3)"));
        ctx.fill_rect(
            0.0,
            0.0,
            f64::from(canvas.width()),
            f64::from(canvas.height()),
        );
    }

    fn view(&self) -> Node<Msg> {
        div![self.view_canvas(), self.view_ui()]
    }

    fn view_canvas(&self) -> Node<Msg> {
        div![canvas![
            el_ref(&self.el_canvas),
            el_key(&"stuck_canvas"),
            attrs! {
                At::Width => px(CANVAS_WIDTH),
                At::Height => px(CANVAS_HEIGHT),
            },
        ]]
    }

    fn view_ui(&self) -> Node<Msg> {
        div![
            div![util::format_duration(self.elapsed)],
            div!["STUCK..."],
            div![button!["Restart", ev(Ev::Click, |_| Msg::Restart)]],
        ]
    }
}
