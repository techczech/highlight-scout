import React from "react";
import ReactDOM from "react-dom/client";
import "@bridle/settings-react/settings.css";
import "./settings/settings-page.css";
import { SettingsApp } from "./settings/SettingsApp";

const params = new URLSearchParams(window.location.search);
const section = params.get("section") ?? undefined;
const action = params.get("action") ?? undefined;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SettingsApp initialSection={section} initialAction={action} />
  </React.StrictMode>,
);
