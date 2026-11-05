use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

pub fn show(parent: &adw::ApplicationWindow, title: &str, path: &std::path::Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };

    let window = adw::Window::builder()
        .title(title)
        .transient_for(parent)
        .modal(true)
        .default_width(640)
        .default_height(600)
        .build();

    let view = gtk::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .wrap_mode(gtk::WrapMode::Word)
        .top_margin(24)
        .bottom_margin(24)
        .left_margin(24)
        .right_margin(24)
        .build();

    let buffer = view.buffer();
    let h1 = heading_tag(&buffer, "h1", 1.6, 18);
    let h2 = heading_tag(&buffer, "h2", 1.3, 14);
    let h3 = heading_tag(&buffer, "h3", 1.12, 10);
    let item = gtk::TextTag::new(Some("item"));
    item.set_indent(16);
    buffer.tag_table().add(&item);

    for line in text.lines() {
        let line = line.trim_end();
        let (tag, body) = if let Some(rest) = line.strip_prefix("### ") {
            (Some(&h3), rest.to_string())
        } else if let Some(rest) = line.strip_prefix("## ") {
            (Some(&h2), rest.to_string())
        } else if let Some(rest) = line.strip_prefix("# ") {
            (Some(&h1), rest.to_string())
        } else if let Some(rest) = line.strip_prefix("- ") {
            (Some(&item), format!("•  {rest}"))
        } else {
            (None, line.to_string())
        };

        let mut start = buffer.end_iter();
        buffer.insert(&mut start, &inline_text(&body));
        let end = buffer.end_iter();
        if let Some(tag) = tag {
            buffer.apply_tag(tag, &start, &end);
        }
    }

    let scroller = gtk::ScrolledWindow::builder()
        .child(&view)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&scroller));
    window.set_content(Some(&toolbar));
    window.present();
    true
}

fn heading_tag(
    buffer: &gtk::TextBuffer,
    name: &str,
    scale: f64,
    pixels_above: i32,
) -> gtk::TextTag {
    let tag = gtk::TextTag::new(Some(name));
    tag.set_scale(scale);
    tag.set_weight(700);
    tag.set_pixels_above_lines(pixels_above);
    tag.set_pixels_below_lines(4);
    buffer.tag_table().add(&tag);
    tag
}

fn inline_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('[') {
        let Some(mid) = rest[start..].find("](") else {
            break;
        };
        let Some(end) = rest[start + mid..].find(')') else {
            break;
        };
        out.push_str(&rest[..start]);
        out.push_str(&rest[start + 1..start + mid]);
        rest = &rest[start + mid + end + 1..];
    }
    out.push_str(rest);
    out.replace("**", "").replace('`', "")
}
