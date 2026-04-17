# Music Recognizer (example plugin stub)

A placeholder illustrating how a music-recognition feature would live as a
plugin rather than a core feature — MoonLens itself does not ship music
recognition because no sustainable free provider exists.

Implement this plugin by:

1. Capturing loopback audio (MoonLens grants the `audio_capture` permission
   if your `plugin.toml` declares it).
2. Fingerprinting and querying the service of your choice (for example,
   AudD or ACRCloud with a user-provided API key).
3. Returning the recognized track as `TaskOutput::Text` or a structured
   `custom` payload.
