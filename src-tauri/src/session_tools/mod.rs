//! Session-Werkzeuge: der Werkzeug-Satz im MCP-Zugang (ADR 046), über den Sessions eines Vorhabens
//! miteinander sprechen. Nur in der Betriebsart Claude. Der Zustand und die Zustellung liegen in
//! `sessions/registry/session_tools.rs`.
pub mod model;
pub mod tools;
