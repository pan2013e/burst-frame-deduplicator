(() => {
  if (typeof window === "undefined") {
    self.addEventListener("install", () => self.skipWaiting());
    self.addEventListener("activate", event => event.waitUntil(self.clients.claim()));
    self.addEventListener("fetch", event => {
      if (event.request.cache === "only-if-cached" && event.request.mode !== "same-origin") return;
      event.respondWith(fetch(event.request).then(response => {
        if (response.status === 0) return response;
        const headers = new Headers(response.headers);
        headers.set("Cross-Origin-Embedder-Policy", "require-corp");
        headers.set("Cross-Origin-Opener-Policy", "same-origin");
        headers.set("Cross-Origin-Resource-Policy", "same-origin");
        return new Response(response.body, {
          status: response.status,
          statusText: response.statusText,
          headers,
        });
      }));
    });
    return;
  }

  if (!window.crossOriginIsolated && "serviceWorker" in navigator) {
    navigator.serviceWorker.register("./coi-serviceworker.js").then(() => {
      if (!navigator.serviceWorker.controller && !sessionStorage.getItem("coi-reload")) {
        sessionStorage.setItem("coi-reload", "1");
        navigator.serviceWorker.addEventListener("controllerchange", () => location.reload(), { once: true });
      }
    }).catch(() => {});
  }
})();
