# Plan Agent

The Plan Agent is a dock on the right side of the window where you describe a
change to the plan in plain words, for example "add a 24' x 30' two-car
garage on the right side with a door into the mudroom". Claude reads the plan
through a set of tools, edits it, and the result lands in your plan as ONE
undo step named "Plan Agent". It is Plan Studio's own feature (Chief
Architect has no counterpart).

## Open it

View > Plan Agent, or the fourth tab in the dock's tab strip. The dock keeps
working when you close it; the result is applied when the run finishes.

## Where the key goes

Edit > Preferences > Plan Agent.

* **Anthropic API key**: masked. It is saved only in your user preferences
  file (`~/.plan-studio/preferences.json`), never in a plan file, and it is
  never shown or logged. "Forget key" clears it.
* Leave the key empty to use the `ANTHROPIC_API_KEY` environment variable.
* **Model**: `claude-opus-5-5` unless you type another model id.
* **Effort**: Low, Medium, High or Extra high; the dock's own effort menu
  changes the same setting.

Without a key the request box is disabled and the dock says "Set your
Anthropic API key in Preferences > Plan Agent", with a link to that page.

## Using the dock

1. Type the request in the box ("Ask to make changes") and press Send, or
   Ctrl+Enter (Cmd+Enter on a Mac).
2. The transcript shows your request, the agent's reply and, under "Working...",
   one row per step it took: a check mark and the tool's name with what it did,
   or a cross with the error. "Thinking..." rows are folded; click to read.
3. **Stop** ends the run after the current call; nothing is applied.
4. When the agent exposes dimensions you can tune (garage width, overhang),
   they appear under **Tweaks** as sliders. Move them and press **Apply
   changes**; the agent re-runs with the new values.
5. **Clear** starts a new conversation.

## Cost line

The footer shows the tokens used by this conversation and an estimate of the
cost, for example `Usage: 12.3k in / 2.1k out . est. $0.08`, and the model.
The estimate uses published list prices and ignores discounts.

## Undo

The agent works on a copy of the plan. When it finishes and changed something,
the copy replaces the plan as a single step: Edit > Undo ("Undo Plan Agent")
puts back everything the request did, and Redo brings it back. A run that
changes nothing leaves no undo step. Edits you make by hand while a run is in
progress are replaced by the agent's result, so wait for it to finish (or press
Stop) before drawing.
