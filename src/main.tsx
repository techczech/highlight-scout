import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import WorkWindow from "./WorkWindow";
import RelatedWindow from "./RelatedWindow";
import { applyTextSize, textSizeOf } from "./settings/appearance";
import { loadView, onSettingsChanged } from "./settings/host";
import "./index.css";

// Text size is the appearance.textSize setting; every window follows it.
loadView().then((v) => applyTextSize(textSizeOf(v.state.settings))).catch(() => {});
onSettingsChanged((v) => applyTextSize(textSizeOf(v.state.settings))).catch(() => {});

// Route by query param: ?work=<id> → work window, ?related=<id> → related
// window, otherwise the main app.
const params = new URLSearchParams(window.location.search);
const workId = params.get("work");
const relatedId = params.get("related");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {workId ? <WorkWindow workId={workId} /> : relatedId ? <RelatedWindow id={relatedId} /> : <App />}
  </React.StrictMode>,
);
