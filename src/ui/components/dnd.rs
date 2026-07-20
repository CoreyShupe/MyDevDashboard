//! Shared drag-and-drop helpers, so every draggable in the app behaves consistently.
//!
//! The one rule they enforce: a dragged item follows the pointer **from the exact point the
//! user grabbed it** — it never snaps its centre to the cursor. Both the ticket cards and the
//! stage columns lift onto a floating layer via [`drag_ghost`]; the payload/handle wiring stays
//! with each caller (they differ), but the visual "pick up and carry" is identical.

use egui::emath::TSTransform;
use egui::{Id, LayerId, Order, Ui, UiBuilder};

/// Paint `render` onto a floating drag layer that tracks the pointer **from the grab point**
/// (no centre-snap). Call this in the `ctx.is_being_dragged(drag_id)` branch of a manual drag,
/// after setting the payload. It sets no payload and does no hit-testing — the caller owns those.
///
/// How the offset works: the ghost is laid out at the item's normal home position this frame,
/// then the whole layer is translated by how far the pointer has moved since the press
/// (`now - press_origin`). At press that delta is zero, so the grabbed point sits under the
/// cursor and stays there as the pointer moves.
pub fn drag_ghost(ui: &mut Ui, drag_id: Id, render: impl FnOnce(&mut Ui)) {
    let layer_id = LayerId::new(Order::Tooltip, drag_id);
    ui.scope_builder(UiBuilder::new().layer_id(layer_id), render);

    let ctx = ui.ctx();
    if let (Some(now), Some(press)) = (
        ctx.pointer_interact_pos(),
        ctx.input(|i| i.pointer.press_origin()),
    ) {
        ctx.transform_layer_shapes(layer_id, TSTransform::from_translation(now - press));
    }
}
