//! External state updates and their shell requests.

use super::*;

impl<S: 'static> Runtime<S> {
    /// Marks app state changed outside a handler (the next frame re-runs the view).
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    /// Changes the state from outside any handler, for data another thread delivered
    /// (`AppHandle::update`). Runs now, so it works while no window shows; the next frame commits
    /// it with the motion `f` asked for.
    pub fn update(&mut self, f: impl FnOnce(&mut S, &mut Tx)) {
        self.update_if(|s, tx| { f(s, tx); true });
    }

    /// `update` for a change that may be no change: `f` returns whether it changed state the
    /// view reads, as `poll` does. On false the view doesn't re-run and no frame is asked for,
    /// while what `f` asked of its `Tx` still happens: clipboard, close and quit, menus and
    /// window frames reach the shell, a scroll or text flight still takes a frame, and a
    /// `wake_after` waits, like `update`'s, for the next frame or hidden poll to schedule it
    /// (a host that runs neither calls `schedule_pending_wakes`). Returns whether the last frame
    /// went out of date: `f` reported a change or asked for work only a frame does.
    ///
    /// Returning false after changing state the view reads leaves the screen stale until the
    /// next change.
    pub fn update_if(&mut self, f: impl FnOnce(&mut S, &mut Tx) -> bool) -> bool {
        // Its wakes count from now (`schedule_pending_wakes`), so its clock reads now too.
        let mut one = Tx { clock: self.now_clock(), ..self.transaction() };
        let changed = crate::tooling::perf_warn::timed("an `update` closure", || f(&mut self.state, &mut one));
        let changed = self.notify_editor_selections(&mut one) || changed;
        let stale = changed || one.frame_work() || one.window_frame.is_some() || one.srgb_blend.is_some_and(|b| b != self.srgb_blend) || one.font_smoothing.is_some_and(|m| m != self.text.font_smoothing())
            || self.changes_families(&one)
            || (one.blur && self.focus.path().is_some());
        self.latch_requests(&mut one);
        merge(&mut self.pending, one);
        self.dirty |= changed;
        stale
    }

    /// Takes a `Tx::minimize` that `what` can't carry out, saying so in debug builds.
    pub(crate) fn ignore_minimize(&mut self, what: &str) {
        if std::mem::take(&mut self.minimize_request) && cfg!(debug_assertions) {
            eprintln!("rune: minimize ignored: {what} can't minimize");
        }
    }

    /// Latches a Tx's shell and clipboard requests for the shell, so `update`, a hidden poll
    /// and `on_close` reach the clipboard without a frame, and applies clipboard requests to a
    /// memory clipboard at once. A secret clear drops an earlier secret copy not yet written.
    pub(in crate::runtime) fn latch_requests(&mut self, tx: &mut Tx) {
        self.run_pick_reads(tx);
        self.shell_request = self.shell_request.max(tx.shell.take());
        self.minimize_request |= std::mem::take(&mut tx.minimize);
        self.titlebar.toggle_maximized |= std::mem::take(&mut tx.toggle_maximized);
        self.titlebar.toggle_full_screen |= std::mem::take(&mut tx.toggle_full_screen);
        for ev in std::mem::take(&mut tx.keys) { self.key(ev); }
        self.latch_playground(tx);
        for prefix in std::mem::take(&mut tx.evict_memos) { self.invalidate_memos_by_prefix(&prefix); }
        if !tx.open_urls.is_empty() { self.open_urls(std::mem::take(&mut tx.open_urls)); }
        if std::mem::take(&mut tx.clear_secret) {
            if self.copy_request.as_ref().is_some_and(crate::ClipboardWrite::is_secret) { self.copy_request = None; }
            self.clear_secret_request = true;
            self.pill_copy = None;
        }
        if let Some(copy) = tx.copy.take() {
            self.pill_copy = (!copy.is_secret())
                .then(|| copy.content().html())
                .flatten()
                .filter(|h| h.contains("<span data-pill="))
                .map(|h| {
                    let frag = crate::TextEdit::from_html(h);
                    (
                        zeroize::Zeroizing::new(copy.content().text().to_owned()),
                        frag.pills().map(|(r, id)| (r, id.to_owned())).collect::<Vec<_>>(),
                    )
                })
                .filter(|(_, pills)| !pills.is_empty());
            self.copy_request = Some(copy);
        }
        if std::mem::take(&mut tx.paste) { self.paste_request = Some(self.paste_target()); }
        for read in tx.clipboard_reads.drain(..).filter_map(|read| read.take()) {
            self.next_clipboard_read += 1;
            self.clipboard_reads.push((self.next_clipboard_read, read));
            self.read_requests.push(self.next_clipboard_read);
        }
        self.popups.extend(tx.popups.drain(..).filter_map(|call| call.take()));
        for request in tx.file_dialogs.drain(..).filter_map(|call| call.take()) {
            self.request_file_dialog(request);
        }
        if let Some(mode) = tx.font_smoothing.take() { self.set_font_smoothing(mode); }
        if let Some((sans, mono)) = tx.font_families.take() { self.set_font_families(&sans, &mono); }
        if let Some(srgb) = tx.srgb_blend.take() { self.set_srgb_blend(srgb); }
        if let Some(rewind) = tx.rewind.take() { self.rewind = rewind; }
        if std::mem::take(&mut tx.blur) {
            if self.focus.path().is_some() {
                self.set_focus(None, self.focus.keyboard());
                self.dirty = true;
            }
            // An explicit blur is not the temporary pointer-down blur of a scrim.
            self.focus.step(focus_machine::FocusEvent::Settled);
        }
        if let Some(request) = tx.focus_to.take() {
            self.focus_request = Some(request);
            self.dirty = true;
        }
        if let Some(title) = tx.title.take() { self.window_title.set(title); }
        if let Some(mut request) = tx.window_frame.take() {
            request.spring = (request.animate && !tx.commit.snaps() && !self.reduced_motion).then(|| tx.motion.map_or(motion::DEFAULT, MotionSource::spring));
            self.window_frame = Some(self.window_frame.unwrap_or_default().merge(request));
        }
        crate::clipboard::flush_memory(self);
    }

}
