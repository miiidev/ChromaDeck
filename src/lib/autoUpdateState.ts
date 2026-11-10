/** Pure state machine for the auto-updater UI. */

export type UpdateState =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "available"; version: string; notes: string | null; date: string | null }
  | {
      status: "downloading";
      version: string;
      notes: string | null;
      date: string | null;
      downloaded: number;
      total: number | null;
      percent: number | null;
    }
  | { status: "ready-to-install"; version: string; notes: string | null; date: string | null }
  | { status: "error"; message: string };

export type UpdateAction =
  | { type: "CHECK_START" }
  | {
      type: "CHECK_SUCCESS";
      payload: { available: boolean; version: string; notes: string | null; date: string | null };
    }
  | { type: "CHECK_FAILURE"; payload: { message: string } }
  | { type: "DOWNLOAD_START" }
  | {
      type: "DOWNLOAD_PROGRESS";
      payload: { downloaded: number; total: number | null; percent: number | null };
    }
  | { type: "DOWNLOAD_SUCCESS" }
  | { type: "DOWNLOAD_FAILURE"; payload: { message: string } }
  | { type: "RESET" };

export function updateStateReducer(
  state: UpdateState,
  action: UpdateAction,
): UpdateState {
  switch (action.type) {
    case "CHECK_START":
      return { status: "checking" };
    case "CHECK_SUCCESS":
      if (!action.payload.available) return { status: "idle" };
      return {
        status: "available",
        version: action.payload.version,
        notes: action.payload.notes,
        date: action.payload.date,
      };
    case "CHECK_FAILURE":
      return { status: "error", message: action.payload.message };
    case "DOWNLOAD_START":
      if (state.status !== "available") return state;
      return {
        status: "downloading",
        version: state.version,
        notes: state.notes,
        date: state.date,
        downloaded: 0,
        total: null,
        percent: null,
      };
    case "DOWNLOAD_PROGRESS":
      if (state.status !== "downloading") return state;
      return {
        ...state,
        downloaded: action.payload.downloaded,
        total: action.payload.total,
        percent: action.payload.percent,
      };
    case "DOWNLOAD_SUCCESS":
      if (state.status !== "downloading") return state;
      return {
        status: "ready-to-install",
        version: state.version,
        notes: state.notes,
        date: state.date,
      };
    case "DOWNLOAD_FAILURE":
      return { status: "error", message: action.payload.message };
    case "RESET":
      return { status: "idle" };
    default:
      return state;
  }
}

export function initialUpdateState(): UpdateState {
  return { status: "idle" };
}
