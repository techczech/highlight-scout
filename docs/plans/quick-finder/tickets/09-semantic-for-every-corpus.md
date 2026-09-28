# 09: Semantic search across every ticked corpus

**What to build:** Dominik (2026-09-28): "we should build semantic options for any archive". scout-core **v0.3.0** adds local multilingual semantic and hybrid search for every corpus. The vectors are in `<corpus>.vectors.sqlite` beside each index; the model is paraphrase-multilingual-MiniLM-L12-v2 in `~/local-models` (HF_HOME).
- Move all scout-core pins to v0.3.0 and enable the engine's `onnx` feature in the app. Attach the embedder via `Engine::with_embedder` so the corpus engine can embed queries.
- The search-box **Keyword / Semantic** switch now means:
  - **Keyword:** full text, as today.
  - **Semantic:** meaning-based across ALL ticked corpora, through the engine.
  - **Default when vectors exist:** the engine's **hybrid** ranking, so exact words still win.
- Semantic no longer unticks Writing and Tweets. Remove the "Highlights only" note, and retire the qmd path once the engine covers highlights. Keep qmd only if some classic semantic behaviour has no engine equivalent, and report it.
- Missing or stale vectors: the app offers "Build meaning index" in the background, stating time and size (about 9 min and 250 MB for the three corpora on this Mac), and never builds silently. Until then, Semantic explains why it is unavailable.
- Latency: the model loads once per app session, not per query.

**Blocked by:** None (scout-core v0.3.0 tagged). **Seams under test:** the search model (mode → engine request: fts | semantic | hybrid); the vector-state UI (missing, building, current); the mount smoke.
**Status:** ready

- [ ] Semantic "the mind as a machine" with Writing + Highlights ticked returns hits from both.
- [ ] Version 0.6.0-preview.6; nothing under the protected write-path files changes.
