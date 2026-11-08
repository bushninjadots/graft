import type { TerminalEvent, TerminalSessionInfo } from "./api";

export type TerminalSession = {
  info: TerminalSessionInfo;
  transcript: string;
  ended: boolean;
  subscribe: (listener: (event: TerminalEvent) => void) => () => void;
};
