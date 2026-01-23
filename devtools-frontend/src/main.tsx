import React from "react";
import ReactDOMClient from "react-dom/client";
import "./index.css";
import {
  LoaderFunction,
  RouterProvider,
  createBrowserRouter,
  useRouteError,
} from "react-router-dom";
import { TelemetryView } from "./components/TelemetryView";
import { Layout } from "./components/Layout";
import { HelmetProvider } from "react-helmet-async";
import { Top } from "./components/Top";
import { Callout, FocusStyleManager, Intent } from "@blueprintjs/core";
import { CommandView } from "./components/CommandView";
import { OldCommandView } from "./components/OldCommandView";
import { buildClient } from "./client";
import type { GrpcClientService } from "./worker";
import { IconNames } from "@blueprintjs/icons";
import { FriendlyError } from "./error";

FocusStyleManager.onlyShowFocusOnTabs();

const root = ReactDOMClient.createRoot(document.getElementById("root")!);

const clientLoader: LoaderFunction = async () => {
  const worker = new SharedWorker(new URL("./worker.ts", import.meta.url), {
    type: "module",
    /* @vite-ignore */
    name: location.origin,
  });
  const client = buildClient<GrpcClientService>(worker);
  const { satelliteSchema } = await client.getSatelliteSchema().catch((err) => {
    throw new FriendlyError(`Failed to get satellite schema`, {
      cause: err,
      details: "Make sure that your tmtc-c2a is running.",
    });
  })!;
  return { client, satelliteSchema };
};

const ErrorBoundary = () => {
  const error = useRouteError();
  console.error(error);
  let title = "Error";
  let description = `${error}`;
  if (error instanceof FriendlyError) {
    title = `${error.message}`;
    description = error.details ?? `${error.cause}`;
  }
  return (
    <div className="grid h-screen place-items-center">
      <div>
        <Callout intent={Intent.DANGER} title={title} icon={IconNames.ERROR}>
          {description}
        </Callout>
      </div>
    </div>
  );
};

const router = createBrowserRouter(
  [
    {
      path: "/",
      element: <Layout />,
      loader: clientLoader,
      errorElement: <ErrorBoundary />,
      children: [
        {
          path: "",
          element: <Top />,
        },
        {
          path: "telemetries/:tmivName",
          element: <TelemetryView />,
        },
        {
          path: "command_new",
          element: <CommandView />,
        },
        {
          path: "command",
          element: <OldCommandView />,
        },
      ],
    },
  ],
  {
    basename: import.meta.env.BASE_URL,
  },
);

root.render(
  <React.StrictMode>
    <HelmetProvider>
      <RouterProvider router={router} />
    </HelmetProvider>
  </React.StrictMode>,
);
