# TUI/CLI UI/UX Audit Report

**Date:** 2026-03-04  
**Auditor:** AI Agent  
**Scope:** End-to-end UI/UX audit of rot TUI and CLI

---

## Executive Summary

The TUI has several critical issues affecting text visibility and flexibility. The primary problems stem from inaccurate content height calculations, fixed-width assumptions, and lack of responsive text wrapping. This audit identifies **15 issues** across 4 severity levels.

### Critical Issues Found: 3
### High Priority Issues: 5
### Medium Priority Issues: 4
### Low Priority Issues: 3

---

## Critical Issues (Severity: 🔴 CRITICAL)

### 1. Inaccurate Content Height Calculation for Word Wrap
**Location:** `crates/rot-tui/src/app.rs:912-927`

**Problem:**
The content height calculation uses a fixed 1.1 multiplier as a "safety margin" for word wrap:
```rust
content_height = (content_height as f32 * 1.1) as u16 + 2;
```

This is inaccurate because:
- Word wrap behavior varies based on actual character widths (Unicode, emojis, etc.)
- The multiplier doesn't account for the actual rendered width including spans
- Long words that can't be broken may require more lines than calculated

**Impact:** Text gets cut off at the bottom, last messages may be partially visible or completely hidden.

**Fix:** Use ratatui's actual line wrapping calculation or calculate based on actual span widths.

---

### 2. Fixed Width Assumption Doesn't Match Rendered Content
**Location:** `crates/rot-tui/src/app.rs:914`

**Problem:**
```rust
let content_width = area.width.saturating_sub(2);
```

But the actual message rendering adds:
- 1 space span (line 831)
- 1 bar_span "▌ " (2-3 characters depending on font)
- Total: ~4 characters of overhead

This mismatch causes incorrect line wrap predictions.

**Impact:** Text wraps at wrong positions, creating visual inconsistencies and incorrect scroll calculations.

**Fix:** Calculate actual usable width accounting for all rendered spans.

---

### 3. Copy Button Non-Functional
**Location:** `crates/rot-tui/src/app.rs:848-866`

**Problem:**
A [Copy] button is rendered at the end of messages:
```rust
let copy_span = Span::styled(
    " [Copy] ",
    Style::default()
        .fg(COLOR_ACCENT)
        .bg(COLOR_CODE_BG)
        .bold(),
);
```

But there's NO click handling for this button in `runner.rs`. Mouse clicks (lines 626-639) only handle:
- Footer access mode toggle
- Clicking anywhere to copy last message

**Impact:** Users see a clickable button that doesn't work, causing confusion and poor UX.

**Fix:** Either implement click detection for the [Copy] button or remove it and use a different UX pattern (e.g., keyboard shortcut hint).

---

## High Priority Issues (Severity: 🟠 HIGH)

### 4. No Horizontal Scroll/Overflow Handling
**Location:** `crates/rot-tui/src/app.rs:782-954`

**Problem:**
The messages area only supports vertical scrolling:
```rust
.scroll((self.scroll_offset, 0))
```

Long lines without natural break points (e.g., long URLs, base64 strings) will either:
- Wrap awkwardly
- Get truncated
- Break the layout

**Impact:** Users can't view long unbreakable content properly.

**Fix:** Add horizontal scroll support or implement smart truncation with expand capability.

---

### 5. Input Box Height Calculation May Clip Content
**Location:** `crates/rot-tui/src/app.rs:687-688`

**Problem:**
```rust
let input_lines = self.input.split('\n').count() as u16;
let input_height = (input_lines + 2).clamp(3, (area.height / 3).max(3));
```

Issues:
- Counts lines but doesn't account for line wrapping
- Maximum of 1/3 screen height may be too restrictive for long prompts
- No visual indicator when input is clipped

**Impact:** Users can't see their full input when writing long multi-line prompts.

**Fix:** Implement proper line wrap counting and consider dynamic resizing or scroll indicator.

---

### 6. Scroll Indicators Overlay Content
**Location:** `crates/rot-tui/src/app.rs:944-953`

**Problem:**
Scroll arrows are rendered on top of content:
```rust
let up_arrow = Paragraph::new(" ▲ ")
    // ...
    frame.render_widget(up_arrow, Rect { x: area.x, y: area.y, ... });
```

These indicators appear at y:0 and y:height-1, overlaying the first and last visible lines.

**Impact:** Content at top/bottom is partially obscured by scroll indicators.

**Fix:** Reserve space for scroll indicators or use a non-overlay approach.

---

### 7. Streaming Text Doesn't Respect Message Box Styling
**Location:** `crates/rot-tui/src/app.rs:872-886`

**Problem:**
Streaming text renders differently than completed messages:
- Completed messages: wrapped in message box with background color
- Streaming text: rendered directly without consistent styling

**Impact:** Visual inconsistency during streaming, jarring transition when complete.

**Fix:** Apply consistent message box styling to streaming text.

---

### 8. Welcome Banner Fixed Width May Break on Narrow Terminals
**Location:** `crates/rot-tui/src/app.rs:274-296`

**Problem:**
The welcome box uses fixed width formatting:
```rust
"             ┃  provider : {:<23}┃\n"
```

On terminals narrower than ~45 columns, this breaks the layout.

**Impact:** Broken UI on small terminals or side-by-side terminal setups.

**Fix:** Make welcome banner responsive or skip on narrow terminals.

---

## Medium Priority Issues (Severity: 🟡 MEDIUM)

### 9. No Visual Feedback for Copy Success
**Location:** `crates/rot-tui/src/app.rs:244-252`, `runner.rs:610-614`

**Problem:**
When copying (via 'y' key or mouse click):
```rust
self.status = "Copied to clipboard!".to_string();
```

But this status appears in the header and is easily missed. No persistent visual feedback.

**Impact:** Users may not know if copy succeeded.

**Fix:** Add temporary visual feedback (e.g., flash effect, toast notification).

---

### 10. Footer Information Overflow
**Location:** `crates/rot-tui/src/app.rs:1030-1130`

**Problem:**
Footer packs lots of information:
- Mode indicator
- Provider:model
- Agent name
- MCP count
- Context percentage
- Token count
- Cost
- Access mode button
- Help hint

On narrow terminals, this overflows. Current padding calculation (line 1117) may result in negative values.

**Impact:** Footer content gets cut off or breaks layout on small screens.

**Fix:** Implement responsive footer with collapsible sections or horizontal scroll.

---

### 11. Approval Dialog May Exceed Screen Bounds
**Location:** `crates/rot-tui/src/app.rs:1212-1278`

**Problem:**
Approval dialog uses percentage-based positioning:
```rust
Constraint::Percentage(30),
Constraint::Min(10),
Constraint::Percentage(30),
```

But doesn't validate that the resulting dialog fits on screen, especially with long tool arguments.

**Impact:** Dialog may be partially off-screen, hiding important information or buttons.

**Fix:** Add bounds checking and scrolling for long content.

---

### 12. No Loading State for Model/Agent Switching
**Location:** `runner.rs:287-314`, `327-352`

**Problem:**
When switching models or agents, there's no visual feedback during provider creation. The UI just hangs until complete.

**Impact:** Users may think the app froze during model switching.

**Fix:** Show loading indicator during provider/agent rebuild.

---

## Low Priority Issues (Severity: 🟢 LOW)

### 13. Markdown Parser Doesn't Handle Nested Formatting
**Location:** `crates/rot-tui/src/app.rs:1442-1505`

**Problem:**
The markdown parser handles bold and code separately but doesn't support nested formatting like `**bold with \`code\` inside**`.

**Impact:** Incorrect rendering of complex markdown.

**Fix:** Implement proper markdown parsing with nesting support (or use a crate).

---

### 14. No Accessibility Features
**Location:** Throughout TUI

**Problem:**
- No screen reader support
- No high contrast mode
- Colors are hardcoded (no theme customization)
- No keyboard shortcuts documentation in-app

**Impact:** Poor accessibility for users with visual impairments.

**Fix:** Add accessibility settings and screen reader hints.

---

### 15. Cursor Position Not Visible in Long Input
**Location:** `crates/rot-tui/src/app.rs:1004-1027`

**Problem:**
When input has many lines and cursor is below visible area:
```rust
let scroll_y = if cursor_y >= visible_height {
    cursor_y.saturating_sub(visible_height) + 1
} else {
    0
};
```

The cursor may still be outside the rendered area due to timing issues.

**Impact:** Users lose cursor position in long inputs.

**Fix:** Ensure cursor is always visible after scroll calculation.

---

## Recommendations

### Immediate Actions (Critical)

1. **Fix content height calculation** - Replace the 1.1 multiplier with actual line counting
2. **Fix width calculation** - Account for all spans in width calculation
3. **Remove or fix [Copy] button** - Either implement functionality or remove

### Short-term (High Priority)

4. Add horizontal scroll or smart truncation
5. Improve input box flexibility
6. Fix scroll indicator overlay
7. Unify streaming/completed message styling
8. Make welcome banner responsive

### Medium-term (Medium Priority)

9. Add visual feedback for clipboard operations
10. Implement responsive footer
11. Add bounds checking for dialogs
12. Add loading states for async operations

### Long-term (Low Priority)

13. Improve markdown parser
14. Add accessibility features
15. Polish cursor visibility

---

## Testing Recommendations

1. **Test on various terminal sizes** - Especially narrow (< 60 cols) and short (< 20 rows)
2. **Test with long content** - Long URLs, code blocks, base64 strings
3. **Test Unicode/Emoji** - Wide characters that affect width calculations
4. **Test scrolling** - Both mouse and keyboard, with various content lengths
5. **Test all keyboard shortcuts** - Document and verify each one
6. **Test copy functionality** - On different platforms

---

## Code Quality Observations

### Positive
- Well-structured theme constants
- Good separation of concerns
- Comprehensive keyboard shortcuts
- Responsive input mode switching

### Needs Improvement
- Magic numbers for calculations (1.1 multiplier, padding values)
- No responsive design patterns
- Limited error handling for UI edge cases
- No unit tests for rendering logic

---

## Conclusion

The rot TUI has a solid foundation but suffers from text visibility issues primarily caused by inaccurate content measurement calculations. The most critical fixes involve:

1. Proper content height calculation based on actual rendered lines
2. Accurate width accounting for all visual elements
3. Making the [Copy] button functional or removing it

Addressing these issues will significantly improve the user experience, especially for users working with long messages or on smaller terminal windows.

**Priority:** Fix critical issues first, then proceed to high priority items. Medium and low priority items can be addressed in future iterations.
