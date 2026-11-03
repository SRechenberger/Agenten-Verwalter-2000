//! Session-Werkzeuge: der MCP-Server des Verwalters, über den Sessions eines Vorhabens miteinander
//! sprechen. Nur in der Betriebsart Claude. Der Zustand und die Zustellung liegen in
//! `sessions/registry/session_tools.rs`.
pub mod model;
pub mod server;
pub mod tools;
