// Support module: the launcher injects this file into the game as `crate::game::enough_items`,
// beside `Game`, so it can use the item panel and the recipe tables the game builds for viewers.
//
// Flow:
// - `tick` (once a client tick): when the world's item/recipe tables appear or change, index them
//   and show every item in the panel beside container screens.
// - `panel_event` (each click, search, key or tab on the panel): filter the list, open a recipe
//   page ("how is it made" / "what is it used for"), switch station tabs, go back, or move a
//   recipe's ingredients into the open crafting grid.
use super::Game;
use super::native_hooks::{
    NativeRecipe, NativeRecipeKind, NativeRecipeLayout, NativeRecipeResult, NativeRecipeStack,
    NativeRecipeView, NativeRecipes,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// The manifest's id: the game only routes panel events of this id to us.
pub const MOD_ID: &str = "recipes";

/// The panel holds at most this many items.
const MAX_ITEMS: usize = 8192;
/// A recipe page holds at most this many recipes per tab.
const MAX_RECIPES: usize = 256;
/// How many pages "Back" remembers.
const MAX_HISTORY: usize = 64;

/// The stations, in tab order, each shown by its block's item.
const STATIONS: [&str; 8] = [
    "minecraft:crafting_table",
    "minecraft:furnace",
    "minecraft:blast_furnace",
    "minecraft:smoker",
    "minecraft:campfire",
    "minecraft:stonecutter",
    "minecraft:smithing_table",
    "minecraft:brewing_stand",
];

fn station(kind: NativeRecipeKind) -> usize {
    match kind {
        NativeRecipeKind::CraftingShaped | NativeRecipeKind::CraftingShapeless => 0,
        NativeRecipeKind::Smelting => 1,
        NativeRecipeKind::Blasting => 2,
        NativeRecipeKind::Smoking => 3,
        NativeRecipeKind::Campfire => 4,
        NativeRecipeKind::Stonecutting => 5,
        NativeRecipeKind::Smithing => 6,
        NativeRecipeKind::Brewing => 7,
    }
}

fn is_crafting(kind: NativeRecipeKind) -> bool {
    matches!(
        kind,
        NativeRecipeKind::CraftingShaped | NativeRecipeKind::CraftingShapeless
    )
}

/// The game's tables, with what the viewer looks up in them.
struct Index {
    tables: Arc<NativeRecipes>,
    /// Each item's key, by its index in `tables.items` (None: the game can't name it).
    keys: Vec<Option<mod_api::ItemKey>>,
    by_key: HashMap<mod_api::ItemKey, u32>,
    /// Each item's name and id in lower case, for the search.
    names: Vec<String>,
    ids: Vec<String>,
    /// For each item, the recipes making it and the recipes taking it, by index in `tables.recipes`.
    produces: Vec<Vec<u32>>,
    uses: Vec<Vec<u32>>,
    stations: [Option<mod_api::ItemKey>; 8],
}

impl Index {
    fn new(tables: Arc<NativeRecipes>) -> Self {
        let n = tables.items.len();
        let keys: Vec<Option<mod_api::ItemKey>> = tables
            .items
            .iter()
            .map(|item| mod_api::__private::item_key(&item.id))
            .collect();
        let by_key = keys
            .iter()
            .enumerate()
            .filter_map(|(i, key)| Some(((*key)?, i as u32)))
            .collect();
        let names = tables.items.iter().map(|i| i.name.to_lowercase()).collect();
        let ids = tables.items.iter().map(|i| i.id.to_lowercase()).collect();
        let mut produces = vec![Vec::new(); n];
        let mut uses = vec![Vec::new(); n];
        for (r, recipe) in tables.recipes.iter().enumerate() {
            let r = r as u32;
            let mut add = |lists: &mut Vec<Vec<u32>>, s: &NativeRecipeStack| {
                if let Some(list) = lists.get_mut(s.item as usize)
                    && list.last() != Some(&r)
                {
                    list.push(r);
                }
            };
            for s in &recipe.results {
                add(&mut produces, s);
            }
            for s in recipe.slots.iter().flatten() {
                add(&mut uses, s);
            }
        }
        let stations = STATIONS.map(mod_api::__private::item_key);
        Self {
            tables,
            keys,
            by_key,
            names,
            ids,
            produces,
            uses,
            stations,
        }
    }

    fn stack(&self, s: &NativeRecipeStack) -> Option<mod_api::Stack> {
        Some(mod_api::Stack {
            item: (*self.keys.get(s.item as usize)?)?,
            count: s.count.max(1),
            components: s.components.clone(),
        })
    }

    fn plain(key: mod_api::ItemKey) -> mod_api::Stack {
        mod_api::Stack {
            item: key,
            count: 1,
            components: String::new(),
        }
    }

    /// Whether item `i` fits every search term: `@mod` (its namespace), `#tag` (one of its tags),
    /// or a piece of its name or id.
    fn matches(&self, i: usize, terms: &[String]) -> bool {
        terms.iter().all(|term| {
            if let Some(ns) = term.strip_prefix('@') {
                let namespace = self.ids[i].split(':').next().unwrap_or("");
                namespace.contains(ns)
            } else if let Some(tag) = term.strip_prefix('#') {
                self.tables.items[i]
                    .tags
                    .iter()
                    .any(|t| t.to_lowercase().contains(tag))
            } else {
                self.names[i].contains(term.as_str()) || self.ids[i].contains(term.as_str())
            }
        })
    }

    /// How the panel draws recipe `r`.
    fn view(&self, r: &NativeRecipe) -> NativeRecipeView {
        let slots: Vec<Vec<mod_api::Stack>> = r
            .slots
            .iter()
            .map(|alts| alts.iter().filter_map(|s| self.stack(s)).collect())
            .collect();
        let (layout, width, height) = match r.kind {
            NativeRecipeKind::CraftingShaped => {
                (NativeRecipeLayout::Grid, r.width.max(1), r.height.max(1))
            }
            NativeRecipeKind::CraftingShapeless => {
                let n = slots.len() as u32;
                let w = match n {
                    0 | 1 => 1,
                    2..=4 => 2,
                    _ => 3,
                };
                (NativeRecipeLayout::Grid, w, n.div_ceil(w).max(1))
            }
            NativeRecipeKind::Smelting
            | NativeRecipeKind::Blasting
            | NativeRecipeKind::Smoking
            | NativeRecipeKind::Campfire => (NativeRecipeLayout::Cooking, 1, 1),
            NativeRecipeKind::Stonecutting => (NativeRecipeLayout::Single, 1, 1),
            NativeRecipeKind::Smithing => (NativeRecipeLayout::Smithing, 1, 1),
            NativeRecipeKind::Brewing => (NativeRecipeLayout::Brewing, 1, 1),
        };
        let note = if r.cooking_time > 0 {
            format!(
                "{}s  {} XP",
                short(r.cooking_time as f32 / 20.0),
                short(r.experience)
            )
        } else {
            String::new()
        };
        NativeRecipeView {
            layout,
            width,
            height,
            slots,
            results: vec![NativeRecipeResult {
                stacks: r.results.iter().filter_map(|s| self.stack(s)).collect(),
                chance: None,
            }],
            fluid_inputs: Vec::new(),
            fluid_outputs: Vec::new(),
            note,
            // The "+" button moves the ingredients into an open crafting grid.
            action: is_crafting(r.kind).then_some(true),
        }
    }
}

/// `2`, `0.35`: a number without trailing zeros.
fn short(x: f32) -> String {
    let s = format!("{x:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// The recipes making the item (R, left click).
    Recipes,
    /// The recipes taking the item (U, right click).
    Uses,
}

/// A recipe page: an item's recipes or uses, split into one tab per station.
#[derive(Clone)]
struct Page {
    item: u32,
    mode: Mode,
    /// (station, recipes there), in station order.
    tabs: Vec<(usize, Vec<u32>)>,
    tab: usize,
}

/// What to ask of the game once the state's lock is released.
enum Out {
    Style,
    Items(Vec<mod_api::Stack>),
    Recipes {
        title: String,
        tabs: Vec<mod_api::Stack>,
        tab: u32,
        views: Vec<NativeRecipeView>,
    },
    /// Leave the recipe page for the item list.
    Close,
    Transfer(String),
}

#[derive(Default)]
struct State {
    index: Option<Index>,
    query: String,
    /// The items the panel lists now, by index in the tables.
    shown: Vec<u32>,
    page: Option<Page>,
    history: Vec<Page>,
}

impl State {
    fn refilter(&mut self) {
        let Some(index) = &self.index else {
            self.shown.clear();
            return;
        };
        let terms: Vec<String> = self
            .query
            .to_lowercase()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        self.shown = (0..index.tables.items.len())
            .filter(|&i| {
                index.keys[i].is_some()
                    && index.tables.items[i].id != "minecraft:air"
                    && index.matches(i, &terms)
            })
            .take(MAX_ITEMS)
            .map(|i| i as u32)
            .collect();
    }

    fn items_out(&self) -> Out {
        let Some(index) = &self.index else {
            return Out::Items(Vec::new());
        };
        Out::Items(
            self.shown
                .iter()
                .filter_map(|&i| index.keys[i as usize].map(Index::plain))
                .collect(),
        )
    }

    fn page_out(&self) -> Out {
        let (Some(index), Some(page)) = (&self.index, &self.page) else {
            return Out::Close;
        };
        let name = &index.tables.items[page.item as usize].name;
        let title = match page.mode {
            Mode::Recipes => format!("Recipes: {name}"),
            Mode::Uses => format!("Uses: {name}"),
        };
        let tabs = page
            .tabs
            .iter()
            .map(|&(station, _)| {
                index.stations[station]
                    .or(index.keys[page.item as usize])
                    .map(Index::plain)
            })
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default();
        let views = page.tabs[page.tab]
            .1
            .iter()
            .map(|&r| index.view(&index.tables.recipes[r as usize]))
            .collect();
        Out::Recipes {
            title,
            tabs,
            tab: page.tab as u32,
            views,
        }
    }

    /// Open item `item`'s recipes or uses; nothing when it has none.
    fn open(&mut self, item: u32, mode: Mode) -> Vec<Out> {
        let Some(index) = &self.index else {
            return Vec::new();
        };
        if self
            .page
            .as_ref()
            .is_some_and(|p| p.item == item && p.mode == mode)
        {
            return Vec::new();
        }
        let list = match mode {
            Mode::Recipes => index.produces.get(item as usize),
            Mode::Uses => index.uses.get(item as usize),
        };
        let mut tabs: Vec<(usize, Vec<u32>)> = Vec::new();
        for &r in list.into_iter().flatten() {
            let s = station(index.tables.recipes[r as usize].kind);
            match tabs.iter_mut().find(|(t, _)| *t == s) {
                Some((_, recipes)) if recipes.len() < MAX_RECIPES => recipes.push(r),
                Some(_) => {}
                None => tabs.push((s, vec![r])),
            }
        }
        if tabs.is_empty() {
            return Vec::new();
        }
        tabs.sort_by_key(|(s, _)| *s);
        if let Some(old) = self.page.take() {
            if self.history.len() == MAX_HISTORY {
                self.history.remove(0);
            }
            self.history.push(old);
        }
        self.page = Some(Page {
            item,
            mode,
            tabs,
            tab: 0,
        });
        vec![self.page_out()]
    }

    fn item_of(&self, stack: &mod_api::Stack) -> Option<u32> {
        self.index.as_ref()?.by_key.get(&stack.item).copied()
    }

    fn handle(&mut self, event: &mod_api::PanelEvent) -> Vec<Out> {
        if self.index.is_none() {
            return Vec::new();
        }
        let mode = |right: bool| if right { Mode::Uses } else { Mode::Recipes };
        match event {
            mod_api::PanelEvent::Search(query) => {
                self.query.clone_from(query);
                self.refilter();
                vec![self.items_out()]
            }
            mod_api::PanelEvent::Clicked { index, right } => {
                match self.shown.get(*index as usize).copied() {
                    Some(item) => self.open(item, mode(*right)),
                    None => Vec::new(),
                }
            }
            mod_api::PanelEvent::Key { key, stack } => {
                let mode = match key.as_str() {
                    "key.keyboard.r" => Mode::Recipes,
                    "key.keyboard.u" => Mode::Uses,
                    _ => return Vec::new(),
                };
                match self.item_of(stack) {
                    Some(item) => self.open(item, mode),
                    None => Vec::new(),
                }
            }
            mod_api::PanelEvent::SlotClicked { stack, right, .. } => match self.item_of(stack) {
                Some(item) => self.open(item, mode(*right)),
                None => Vec::new(),
            },
            mod_api::PanelEvent::Action(i) => {
                let (Some(index), Some(page)) = (&self.index, &self.page) else {
                    return Vec::new();
                };
                let Some(&r) = page.tabs[page.tab].1.get(*i as usize) else {
                    return Vec::new();
                };
                let recipe = &index.tables.recipes[r as usize];
                if !is_crafting(recipe.kind) {
                    return Vec::new();
                }
                // The game sends Back after a transfer: let it close the page, not step back.
                self.history.clear();
                vec![Out::Transfer(recipe.id.clone())]
            }
            mod_api::PanelEvent::Tab(t) => {
                let Some(page) = self.page.as_mut() else {
                    return Vec::new();
                };
                if (*t as usize) >= page.tabs.len() || page.tab == *t as usize {
                    return Vec::new();
                }
                page.tab = *t as usize;
                vec![self.page_out()]
            }
            mod_api::PanelEvent::Back => {
                self.page = self.history.pop();
                if self.page.is_some() {
                    vec![self.page_out()]
                } else {
                    vec![Out::Close]
                }
            }
            _ => Vec::new(),
        }
    }
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// Runs `f` on the state; the lock is released before the game is called, so a game call that
/// raises another panel event cannot deadlock.
fn with_state(f: impl FnOnce(&mut State) -> Vec<Out>) -> Vec<Out> {
    let mut guard = STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    f(guard.get_or_insert_with(State::default))
}

fn apply(game: &mut Game<'_>, outs: Vec<Out>) {
    for out in outs {
        match out {
            Out::Style => game.panel_style(
                MOD_ID,
                mod_api::PanelStyle::Classic {
                    centred_search: false,
                    window_width: 0,
                },
            ),
            Out::Items(items) => game.panel_items(MOD_ID, items),
            Out::Recipes {
                title,
                tabs,
                tab,
                views,
            } => game.native_panel_recipes(MOD_ID, &title, tabs, tab, views),
            Out::Close => game.native_panel_recipes(MOD_ID, "", Vec::new(), 0, Vec::new()),
            Out::Transfer(recipe) => game.transfer_recipe_id(&recipe, false),
        }
    }
}

/// Once a client tick: (re)index the tables when they appear or change, and list every item.
pub fn tick(game: &mut Game<'_>) {
    let tables = game.native_recipes();
    let outs = with_state(|state| {
        let Some(tables) = tables else {
            // Outside a world: forget everything, so the next world lists its items again.
            if state.index.is_some() {
                *state = State::default();
            }
            return Vec::new();
        };
        if state
            .index
            .as_ref()
            .is_some_and(|index| Arc::ptr_eq(&index.tables, &tables))
        {
            return Vec::new();
        }
        state.index = Some(Index::new(tables));
        state.page = None;
        state.history.clear();
        state.refilter();
        vec![Out::Style, state.items_out()]
    });
    apply(game, outs);
}

/// The player did `event` on mod `mod_id`'s panel.
pub fn panel_event(game: &mut Game<'_>, mod_id: &str, event: &mod_api::PanelEvent) {
    if mod_id != MOD_ID {
        return;
    }
    let outs = with_state(|state| state.handle(event));
    apply(game, outs);
}
