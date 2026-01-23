import React from "react";
import ReactDOMClient from "react-dom/client";
import "./index.css";
import { ChartsView } from "./ChartsView";
import { FocusStyleManager } from "@blueprintjs/core";

FocusStyleManager.onlyShowFocusOnTabs();

const root = ReactDOMClient.createRoot(document.getElementById("root")!);

root.render(
  <React.StrictMode>
    <ChartsView />
  </React.StrictMode>,
);
