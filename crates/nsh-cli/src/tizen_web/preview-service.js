// The background service of a Tizen web app's personal Smart Hub preview
// (`nsh.json` `tizen.smartHubPreview: "service"`), which `nsh package
// --target tizen-web` puts in the package as `service/preview.js` and
// declares in `config.xml` (`<tizen:service>`, `use.preview` `bg_service`).
// Samsung's personal preview is set from such a service: the app's page
// (`nsh/tizen`'s `setPreviewData`, through the TV shim) launches it with the
// preview's JSON under the key `preview`; it hands that to
// `webapis.preview.setPreviewData` and ends, as Samsung asks (the TV ends a
// service that runs on after 5 minutes). Unverified on a TV.
// Written without newer syntax: the TV's service runtime may be older
// than its browser engine.
"use strict";

/** Samsung's `webapis` in a service: the global, else the service build. */
function samsungApis() {
  if (typeof globalThis !== "undefined" && globalThis.webapis) return globalThis.webapis;
  try {
    return require("$WEBAPIS/service/webapis.js");
  } catch (error) {
    return null;
  }
}

/** The preview's JSON the app sent with the request, if any. */
function requested(app) {
  var request = app.getRequestedAppControl();
  var data = (request && request.appControl && request.appControl.data) || [];
  for (var i = 0; i < data.length; i++) {
    if (data[i].key === "preview" && data[i].value && data[i].value.length) return data[i].value[0];
  }
  return null;
}

function message(error) {
  return (error && error.message) || String(error);
}

module.exports.onStart = function () {};

module.exports.onRequest = function () {
  var app = tizen.application.getCurrentApplication();
  var done = function () { app.exit(); };
  var json = requested(app);
  var api = samsungApis();
  var preview = api && api.preview;
  if (json == null || !preview) {
    console.error(json == null ? "nsh preview: no preview data" : "nsh preview: no webapis.preview");
    done();
    return;
  }
  try {
    preview.setPreviewData(json, done, function (error) {
      console.error("nsh preview: " + message(error));
      done();
    });
  } catch (error) {
    console.error("nsh preview: " + message(error));
    done();
  }
};

module.exports.onExit = function () {};
