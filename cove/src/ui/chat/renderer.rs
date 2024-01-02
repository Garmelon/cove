use std::cmp::Ordering;

use async_trait::async_trait;
use toss::Size;

use super::blocks::{Blocks, Range};

#[async_trait]
pub trait Renderer<Id> {
    type Error;

    fn size(&self) -> Size;
    fn scrolloff(&self) -> i32;

    fn blocks(&self) -> &Blocks<Id>;
    fn blocks_mut(&mut self) -> &mut Blocks<Id>;
    fn into_blocks(self) -> Blocks<Id>;

    async fn expand_top(&mut self) -> Result<(), Self::Error>;
    async fn expand_bottom(&mut self) -> Result<(), Self::Error>;
}

/// A range of all the lines that are visible given the renderer's size.
pub fn visible_area<Id, R>(r: &R) -> Range<i32>
where
    R: Renderer<Id>,
{
    let height: i32 = r.size().height.into();
    Range::new(0, height)
}

/// The renderer's visible area, reduced by its scrolloff at the top and bottom.
fn scroll_area<Id, R>(r: &R) -> Range<i32>
where
    R: Renderer<Id>,
{
    let range = visible_area(r);
    let scrolloff = r.scrolloff();
    let top = range.top + scrolloff;
    let bottom = top.max(range.bottom - scrolloff);
    Range::new(top, bottom)
}

/// Compute a delta that makes the object partially or fully overlap the area
/// when added to the object. This delta should be as close to zero as possible.
///
/// If the object has a height of zero, it must be within the area or exactly on
/// its border to be considered overlapping.
///
/// If the object has a nonzero height, at least one line of the object must be
/// within the area for the object to be considered overlapping.
fn overlap_delta(area: Range<i32>, object: Range<i32>) -> i32 {
    assert!(object.top <= object.bottom, "object range not well-formed");
    assert!(area.top <= area.bottom, "area range not well-formed");

    if object.top == object.bottom || area.top == area.bottom {
        // Delta that moves the object.bottom to area.top. If this is positive,
        // we need to move the object because it is too high.
        let move_to_top = area.top - object.bottom;

        // Delta that moves the object.top to area.bottom. If this is negative,
        // we need to move the object because it is too low.
        let move_to_bottom = area.bottom - object.top;

        // move_to_top <= move_to_bottom because...
        //
        // Case 1: object.top == object.bottom
        // Premise follows from rom area.top <= area.bottom
        //
        // Case 2: area.top == area.bottom
        // Premise follows from object.top <= object.bottom
        0.clamp(move_to_top, move_to_bottom)
    } else {
        // Delta that moves object.bottom one line below area.top. If this is
        // positive, we need to move the object because it is too high.
        let move_to_top = (area.top + 1) - object.bottom;

        // Delta that moves object.top one line above area.bottom. If this is
        // negative, we need to move the object because it is too low.
        let move_to_bottom = (area.bottom - 1) - object.top;

        // move_to_top <= move_to_bottom because...
        //
        // We know that area.top < area.bottom and object.top < object.bottom,
        // otherwise we'd be in the previous `if` branch.
        //
        // We get the largest value for move_to_top if area.top is largest and
        // object.bottom is smallest. We get the smallest value for
        // move_to_bottom if area.bottom is smallest and object.top is largest.
        //
        // This means that the worst case scenario is when area.top and
        // area.bottom as well as object.top and object.bottom are closest
        // together. In other words:
        //
        // area.top + 1 == area.bottom
        // object.top + 1 == object.bottom
        //
        // Inserting that into our formulas for move_to_top and move_to_bottom,
        // we get:
        //
        // move_to_top = (area.top + 1) - (object.top + 1) = area.top + object.top
        // move_to_bottom = (area.top + 1 - 1) - object.top = area.top + object.top
        0.clamp(move_to_top, move_to_bottom)
    }
}

pub fn overlaps(area: Range<i32>, object: Range<i32>) -> bool {
    overlap_delta(area, object) == 0
}

/// Move the object such that it overlaps the area.
fn overlap(area: Range<i32>, object: Range<i32>) -> Range<i32> {
    object.shifted(overlap_delta(area, object))
}

/// Compute a delta that makes the object fully overlap the area when added to
/// the object. This delta should be as close to zero as possible.
///
/// If the object is higher than the area, it should be moved such that
/// object.top == area.top.
fn full_overlap_delta(area: Range<i32>, object: Range<i32>) -> i32 {
    assert!(object.top <= object.bottom, "object range not well-formed");
    assert!(area.top <= area.bottom, "area range not well-formed");

    // Delta that moves object.top to area.top. If this is positive, we need to
    // move the object because it is too high.
    let move_to_top = area.top - object.top;

    // Delta that moves object.bottom to area.bottom. If this is negative, we
    // need to move the object because it is too low.
    let move_to_bottom = area.bottom - object.bottom;

    // If the object is higher than the area, move_to_top becomes larger than
    // move_to_bottom. In that case, this function should return move_to_top.
    0.min(move_to_bottom).max(move_to_top)
}

async fn expand_upwards_until<Id, R>(r: &mut R, top: i32) -> Result<(), R::Error>
where
    R: Renderer<Id>,
{
    loop {
        let blocks = r.blocks();
        if blocks.end().top || blocks.range().top <= top {
            break;
        }

        r.expand_top().await?;
    }

    Ok(())
}

async fn expand_downwards_until<Id, R>(r: &mut R, bottom: i32) -> Result<(), R::Error>
where
    R: Renderer<Id>,
{
    loop {
        let blocks = r.blocks();
        if blocks.end().bottom || blocks.range().bottom >= bottom {
            break;
        }

        r.expand_bottom().await?;
    }

    Ok(())
}

pub async fn expand_to_fill_visible_area<Id, R>(r: &mut R) -> Result<(), R::Error>
where
    R: Renderer<Id>,
{
    let area = visible_area(r);
    expand_upwards_until(r, area.top).await?;
    expand_downwards_until(r, area.bottom).await?;
    Ok(())
}

/// Expand blocks such that the screen is full for any offset where the
/// specified block is visible. The block must exist.
pub async fn expand_to_fill_screen_around_block<Id, R>(r: &mut R, id: &Id) -> Result<(), R::Error>
where
    Id: Eq,
    R: Renderer<Id>,
{
    let screen = visible_area(r);
    let (block, _) = r.blocks().find_block(id).expect("no block with that id");

    let top = overlap(block, screen.with_bottom(block.top)).top;
    let bottom = overlap(block, screen.with_top(block.bottom)).bottom;

    expand_upwards_until(r, top).await?;
    expand_downwards_until(r, bottom).await?;

    Ok(())
}

/// Scroll so that the top of the block is at the specified value. Returns
/// `true` if successful, or `false` if the block could not be found.
pub fn scroll_to_set_block_top<Id, R>(r: &mut R, id: &Id, top: i32) -> bool
where
    Id: Eq,
    R: Renderer<Id>,
{
    if let Some((range, _)) = r.blocks().find_block(id) {
        let delta = top - range.top;
        r.blocks_mut().shift(delta);
        true
    } else {
        false
    }
}

pub fn scroll_so_block_is_centered<Id, R>(r: &mut R, id: &Id)
where
    Id: Eq,
    R: Renderer<Id>,
{
    let area = visible_area(r);
    let (range, block) = r.blocks().find_block(id).expect("no block with that id");
    let focus = block.focus(range);
    let focus_height = focus.bottom - focus.top;
    let top = (area.top + area.bottom - focus_height) / 2;
    r.blocks_mut().shift(top - range.top);
}

pub fn scroll_blocks_fully_above_screen<Id, R>(r: &mut R)
where
    R: Renderer<Id>,
{
    let area = visible_area(r);
    let blocks = r.blocks_mut();
    let delta = area.top - blocks.range().bottom;
    blocks.shift(delta);
}

pub fn scroll_blocks_fully_below_screen<Id, R>(r: &mut R)
where
    R: Renderer<Id>,
{
    let area = visible_area(r);
    let blocks = r.blocks_mut();
    let delta = area.bottom - blocks.range().top;
    blocks.shift(delta);
}

pub fn scroll_so_block_focus_overlaps_scroll_area<Id, R>(r: &mut R, id: &Id) -> bool
where
    Id: Eq,
    R: Renderer<Id>,
{
    if let Some((range, block)) = r.blocks().find_block(id) {
        let area = scroll_area(r);
        let delta = overlap_delta(area, block.focus(range));
        r.blocks_mut().shift(delta);
        true
    } else {
        false
    }
}

pub fn scroll_so_block_focus_fully_overlaps_scroll_area<Id, R>(r: &mut R, id: &Id) -> bool
where
    Id: Eq,
    R: Renderer<Id>,
{
    if let Some((range, block)) = r.blocks().find_block(id) {
        let area = scroll_area(r);
        let delta = full_overlap_delta(area, block.focus(range));
        r.blocks_mut().shift(delta);
        true
    } else {
        false
    }
}

pub fn clamp_scroll_biased_downwards<Id, R>(r: &mut R)
where
    R: Renderer<Id>,
{
    let area = visible_area(r);
    let blocks = r.blocks().range();

    // Delta that moves blocks.top to the top of the screen. If this is
    // negative, we need to move the blocks because they're too low.
    let move_to_top = area.top - blocks.top;

    // Delta that moves blocks.bottom to the bottom of the screen. If this is
    // positive, we need to move the blocks because they're too high.
    let move_to_bottom = area.bottom - blocks.bottom;

    // If the screen is higher, the blocks should rather be moved to the bottom
    // than the top because of the downwards bias.
    let delta = 0.min(move_to_top).max(move_to_bottom);
    r.blocks_mut().shift(delta);
}

pub fn find_cursor_starting_at<'a, Id, R>(r: &'a R, id: &Id) -> Option<&'a Id>
where
    Id: Eq,
    R: Renderer<Id>,
{
    let area = scroll_area(r);
    let (range, block) = r.blocks().find_block(id)?;
    let delta = overlap_delta(area, block.focus(range));
    match delta.cmp(&0) {
        Ordering::Equal => Some(block.id()),

        // Blocks must be scrolled downwards to become visible, meaning the
        // cursor must be above the visible area.
        Ordering::Greater => r
            .blocks()
            .iter()
            .filter(|(_, block)| block.can_be_cursor())
            .find(|(range, block)| overlaps(area, block.focus(*range)))
            .map(|(_, block)| block.id()),

        // Blocks must be scrolled upwards to become visible, meaning the cursor
        // must be below the visible area.
        Ordering::Less => r
            .blocks()
            .iter()
            .rev()
            .filter(|(_, block)| block.can_be_cursor())
            .find(|(range, block)| overlaps(area, block.focus(*range)))
            .map(|(_, block)| block.id()),
    }
}
