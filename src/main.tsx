import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import WorkWindow from "./WorkWindow";
import "./index.css";

// Route: ?work=<id> renders a standalone work window; otherwise the main app.
const params = new URLSearchParams(window.location.search);
const workId = params.get("work");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {workId ? <WorkWindow workId={workId} /> : <App />}
  </React.StrictMode>,
);
