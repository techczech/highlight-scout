import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import WorkWindow from "./WorkWindow";
import RelatedWindow from "./RelatedWindow";
import { applyTextSize, getTextSize } from "./lib/textsize";
import "./index.css";

applyTextSize(getTextSize());

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
