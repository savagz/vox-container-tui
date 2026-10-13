use crossterm::event::{KeyCode, KeyEvent};

use crate::deps::{self, DepReport};
use crate::i18n::Lang;

/// `Gate` es el wizard a pantalla completa que corre antes del tablero; `Panel`
/// es el mismo estado reutilizado dentro de la pestaña `SYSTEM CHECK` — ambos
/// comparten exactamente el mismo `DepReport` y el mismo render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemCheckMode {
    Gate,
    Panel,
}

#[derive(Debug, Clone)]
pub enum SystemCheckOutcome {
    None,
    Recheck,
    RunFix(Vec<String>),
    /// Sólo se emite en modo `Gate`: entrar al tablero igual, con chequeos
    /// bloqueantes en rojo (modo degradado, decisión explícita del usuario).
    Continue,
    /// Sólo se emite en modo `Gate`: cancelar y terminar el proceso.
    Cancel,
}

#[derive(Debug, Clone)]
pub struct SystemCheckState {
    pub report: DepReport,
    pub selected: usize,
    pub notice: Option<String>,
    pub lang: Lang,
}

impl SystemCheckState {
    pub fn new(lang: Lang) -> Self {
        Self {
            report: deps::probe(lang),
            selected: 0,
            notice: None,
            lang,
        }
    }

    pub fn recheck(&mut self) {
        self.report = deps::probe(self.lang);
        self.notice = None;
    }

    /// Cambia el idioma y re-corre el probe para regenerar los textos de los
    /// chequeos en el idioma nuevo (se generan una sola vez, no en cada render).
    pub fn set_lang(&mut self, lang: Lang) {
        if self.lang != lang {
            self.lang = lang;
            self.recheck();
        }
    }

    pub fn on_key(&mut self, key: KeyEvent, mode: SystemCheckMode) -> SystemCheckOutcome {
        match key.code {
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                SystemCheckOutcome::None
            }
            KeyCode::Down => {
                self.selected = (self.selected + 1).min(self.report.checks.len().saturating_sub(1));
                SystemCheckOutcome::None
            }
            KeyCode::Char('r') => SystemCheckOutcome::Recheck,
            KeyCode::Enter => match self.report.get(self.selected).and_then(|c| c.state.fix()) {
                Some(fix) => SystemCheckOutcome::RunFix(fix.to_vec()),
                None => SystemCheckOutcome::None,
            },
            KeyCode::Char('a') if mode == SystemCheckMode::Gate => SystemCheckOutcome::Continue,
            KeyCode::Esc if mode == SystemCheckMode::Gate => SystemCheckOutcome::Cancel,
            _ => SystemCheckOutcome::None,
        }
    }
}

impl Default for SystemCheckState {
    fn default() -> Self {
        Self::new(Lang::default())
    }
}
