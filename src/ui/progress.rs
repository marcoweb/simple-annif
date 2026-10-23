use crate::services::processing::DocumentResult;
use eframe::egui;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A monotonic clock stopped by the worker, even when its tab is hidden.
#[derive(Clone)]
pub struct ActivityTimer {
    started: Instant,
    finished: Arc<Mutex<Option<Instant>>>,
}

impl ActivityTimer {
    pub fn start() -> Self {
        Self {
            started: Instant::now(),
            finished: Arc::new(Mutex::new(None)),
        }
    }

    pub fn finish(&self) {
        self.finished
            .lock()
            .unwrap()
            .get_or_insert_with(Instant::now);
    }

    fn elapsed_at(&self, now: Instant) -> Duration {
        self.finished
            .lock()
            .unwrap()
            .unwrap_or(now)
            .saturating_duration_since(self.started)
    }

    pub fn show(&self, ui: &mut egui::Ui) {
        ui.label(format!(
            "Tempo decorrido: {}",
            format_duration(self.elapsed_at(Instant::now()))
        ));
    }
}

fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

/// Only document completions provide a measured fraction of the entire task.
#[derive(Default)]
pub struct BatchProgress {
    pub total: Option<usize>,
    processed: usize,
    failed: usize,
    cancelled: usize,
    samples: usize,
    sample_seconds: f64,
    last_finished: Option<Instant>,
}

impl BatchProgress {
    pub fn record(&mut self, result: &DocumentResult, finished: Instant) {
        if result.cancelled {
            self.cancelled += 1;
            return;
        }
        self.processed += 1;
        self.last_finished = Some(finished);
        if result.error.is_some() {
            self.failed += 1;
        } else if result.duration_ms > 0 {
            self.samples += 1;
            self.sample_seconds += result.duration_ms as f64 / 1000.0;
        }
    }

    fn remaining_at(&self, now: Instant) -> Option<Duration> {
        let remaining = self.total?.saturating_sub(self.processed + self.cancelled);
        if remaining == 0 || self.samples == 0 || self.cancelled > 0 {
            return None;
        }
        let average = self.sample_seconds / self.samples as f64;
        let current = now
            .saturating_duration_since(self.last_finished?)
            .as_secs_f64();
        // Do not show zero while a slower document is still running.
        if current >= average {
            return None;
        }
        Duration::try_from_secs_f64(average * remaining as f64 - current).ok()
    }

    pub fn show(&self, ui: &mut egui::Ui, busy: bool, cancelling: bool) {
        let Some(total) = self.total else {
            if busy {
                ui.label("Preparando lista de documentos…");
            }
            return;
        };
        ui.label(format!(
            "Processados: {} de {total} · Falhas: {} · Cancelados/não processados: {}",
            self.processed, self.failed, self.cancelled
        ));
        if total > 0 {
            ui.add(egui::ProgressBar::new(self.processed as f32 / total as f32).show_percentage());
        }
        if !busy || cancelling {
            return;
        }
        if self.processed >= total {
            ui.label("Finalizando…");
        } else if let Some(remaining) = self.remaining_at(Instant::now()) {
            // Round up rather than displaying 00:00:00 for a positive estimate.
            let rounded = Duration::from_secs(
                remaining
                    .as_secs()
                    .saturating_add(u64::from(remaining.subsec_nanos() > 0)),
            );
            ui.label(format!(
                "Tempo restante estimado: cerca de {}",
                format_duration(rounded)
            ));
            ui.small("Estimativa pela média dos documentos concluídos com sucesso; pode variar conforme o tamanho dos próximos textos.");
        } else if self.samples == 0 {
            ui.label("Tempo restante: aguardando o primeiro documento concluído com sucesso.");
        } else {
            ui.label(
                "Tempo restante: recalculando; o documento atual está demorando mais que a média.",
            );
        }
    }
}

/// Known messages from the bundled Annif 1.4.2. Stages are not percentages.
pub fn phase_from_log(line: &str) -> Option<&'static str> {
    let text = line
        .strip_prefix("[STDOUT] ")
        .or_else(|| line.strip_prefix("[STDERR] "))?;
    let text = text
        .strip_prefix("Backend ")
        .and_then(|message| message.split_once(": "))
        .map_or(text, |(_, message)| message);
    match text {
        "starting train" => Some("Iniciando treinamento"),
        "preparing training data" | "creating vectorizer" | "creating train file" => {
            Some("Preparando dados de treinamento")
        }
        "reusing cached training data from previous run"
        | "Reusing cached training data from previous run." => {
            Some("Lendo dados de treinamento preparados")
        }
        "training model" | "transforming subject corpus" => Some("Treinando modelo"),
        "saving model" | "saving tf-idf matrix" => Some("Salvando modelo"),
        "updating existing subject index" | "creating subject index" => {
            Some("Preparando índice do vocabulário")
        }
        _ if text.starts_with("Loading vocabulary from ") => Some("Lendo vocabulário"),
        _ if text.starts_with("saving vocabulary into SKOS file ") => Some("Salvando vocabulário"),
        _ => None,
    }
}

pub fn show_indeterminate(ui: &mut egui::Ui, phase: &str, cancelling: bool) {
    if !cancelling {
        ui.label(format!("Etapa: {phase}"));
        ui.small("Tempo restante indisponível: esta operação não informa progresso total.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(duration_ms: u128) -> DocumentResult {
        DocumentResult {
            path: "D1.txt".into(),
            project_id: "teste".into(),
            suggestions: vec![],
            error: None,
            cancelled: false,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms,
        }
    }

    #[test]
    fn estimate_uses_successful_documents_and_recalculates_when_overdue() {
        let now = Instant::now();
        let mut progress = BatchProgress {
            total: Some(4),
            ..Default::default()
        };
        assert_eq!(progress.remaining_at(now), None);
        progress.record(&document(10_000), now);
        assert_eq!(
            progress.remaining_at(now + Duration::from_secs(2)),
            Some(Duration::from_secs(28))
        );
        assert_eq!(progress.remaining_at(now + Duration::from_secs(11)), None);
        let mut failed = document(1);
        failed.error = Some("falha rápida".into());
        progress.record(&failed, now);
        assert_eq!(progress.remaining_at(now), Some(Duration::from_secs(20)));
        let mut cancelled = document(0);
        cancelled.cancelled = true;
        progress.record(&cancelled, now);
        assert_eq!(progress.remaining_at(now), None);
        assert_eq!(progress.processed, 2);
        assert_eq!(progress.cancelled, 1);
        assert_eq!(progress.failed, 1);
        progress.record(&document(10_000), now);
        assert_eq!(progress.remaining_at(now), None);
    }

    #[test]
    fn clock_freezes_at_worker_completion_and_stages_do_not_guess_progress() {
        let timer = ActivityTimer::start();
        let worker = timer.clone();
        *worker.finished.lock().unwrap() = Some(timer.started + Duration::from_secs(65));
        assert_eq!(
            timer.elapsed_at(timer.started + Duration::from_secs(600)),
            Duration::from_secs(65)
        );
        worker.finish(); // repeated completion must not alter the result
        assert_eq!(
            format_duration(timer.elapsed_at(Instant::now())),
            "00:01:05"
        );
        assert_eq!(format_duration(Duration::from_secs(3661)), "01:01:01");
        assert_eq!(
            phase_from_log("[STDERR] Backend tfidf: saving model"),
            Some("Salvando modelo")
        );
        assert_eq!(
            phase_from_log("[STDOUT] Loading vocabulary from RDF file x ..."),
            Some("Lendo vocabulário")
        );
        assert_eq!(phase_from_log("[STDERR] unknown progress: 98%"), None);
        assert_eq!(
            ActivityTimer::start().finished.lock().unwrap().as_ref(),
            None
        );
    }
}
