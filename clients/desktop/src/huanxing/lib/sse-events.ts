/**
 * SSE event listener for Sidecar `/api/events`.
 *
 * Uses relative URL so Vite proxy forwards to sidecar.
 */

export type AgentSwitchedHandler = (payload: { agent: string; model: string }) => void;
export type SessionUpdatedHandler = (payload: { session_id: string; title: string }) => void;

interface SseHandlers {
  onAgentSwitched?: AgentSwitchedHandler;
  onSessionUpdated?: SessionUpdatedHandler;
}

/** Connect to the Sidecar SSE event stream. Returns a cleanup function. */
export function connectSseEvents(handlers: SseHandlers): () => void {
  const url = '/api/events';  // relative — Vite proxy to sidecar
  const es = new EventSource(url);

  es.onmessage = (ev) => {
    try {
      const data = JSON.parse(ev.data);
      if (data.type === 'agent_switched' && handlers.onAgentSwitched) {
        handlers.onAgentSwitched({
          agent: data.agent,
          model: data.model,
        });
      }
      if (data.type === 'session_updated' && handlers.onSessionUpdated) {
        handlers.onSessionUpdated({
          session_id: data.session_id,
          title: data.title,
        });
      }
    } catch {
      // Ignore non-JSON events
    }
  };

  es.onerror = () => {
    // EventSource auto-reconnects; nothing to do.
  };

  return () => {
    es.close();
  };
}
