// Bevy/winit attaches keyboard input to its canvas, while panel-kit owns the
// surrounding DOM. Forward unmodified gameplay keys from ordinary panel
// chrome to the canvas without stealing keys from editable form controls.
(function () {
    const forwarded = new Set();
    const gameCodes = new Set([
        "KeyW", "KeyA", "KeyS", "KeyD",
        "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight",
        "KeyF", "KeyG", "KeyH", "KeyI", "KeyJ", "KeyK", "KeyM",
        "KeyN", "KeyP", "KeyR", "KeyT", "KeyU", "KeyV", "KeyY",
        "BracketLeft", "BracketRight", "Equal", "Minus", "Enter",
        "Escape", "Backspace", "Space", "F6",
    ]);

    function consumesGameKey(target, code) {
        if (!(target instanceof Element)) return false;
        if (target.closest("input, textarea, select, [contenteditable=true]")) return true;
        // Keep Enter on a focused button as an activation gesture. Letter,
        // arrow, and bracket flight controls can still pass through panel-kit
        // traffic lights and command buttons after they have been clicked.
        return Boolean(target.closest("button")) && (code === "Enter" || code === "Space");
    }

    function cloneForCanvas(event) {
        return new KeyboardEvent(event.type, {
            key: event.key,
            code: event.code,
            location: event.location,
            repeat: event.repeat,
            ctrlKey: event.ctrlKey,
            shiftKey: event.shiftKey,
            altKey: event.altKey,
            metaKey: event.metaKey,
            bubbles: true,
            cancelable: true,
        });
    }

    function forward(event) {
        if (!event.isTrusted || !gameCodes.has(event.code)) return;
        if (event.ctrlKey || event.altKey || event.metaKey) return;

        const canvas = document.getElementById("bevy");
        if (!canvas || event.target === canvas) return;

        const releasingForwardedKey = event.type === "keyup" && forwarded.has(event.code);
        if (consumesGameKey(event.target, event.code) && !releasingForwardedKey) return;

        canvas.dispatchEvent(cloneForCanvas(event));
        if (event.type === "keydown") forwarded.add(event.code);
        else forwarded.delete(event.code);
        event.preventDefault();
    }

    window.addEventListener("keydown", forward, true);
    window.addEventListener("keyup", forward, true);
})();
