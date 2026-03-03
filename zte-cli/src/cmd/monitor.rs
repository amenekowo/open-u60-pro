use std::io;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Args as ClapArgs;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseEventKind, EnableMouseCapture, DisableMouseCapture};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::prelude::CrosstermBackend;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;

use zte_lib::device::DeviceShell;
use zte_lib::signal::collector::SignalCollector;
use zte_lib::signal::types::{CollectionMethod, SignalSnapshot};

use crate::cmd::ShellArgs;
use crate::ui::csv_logger::CsvLogger;
use crate::ui::panels;

#[derive(ClapArgs)]
pub struct Args {
    /// Poll interval in seconds
    #[arg(short, long, default_value_t = 2)]
    interval: u64,

    /// CSV log file path
    #[arg(long)]
    csv: Option<String>,

    /// Connection args (HTTP default, --ssh or --adb for shell)
    #[command(flatten)]
    shell: ShellArgs,

    /// Data collection method
    #[arg(short, long, default_value = "auto")]
    method: String,

    /// Run duration in seconds (0 = indefinite)
    #[arg(long, default_value_t = 0)]
    duration: u64,

    /// Show debug info (raw rate values, speed source)
    #[arg(short = 'D', long)]
    debug: bool,
}

pub fn run(args: Args) -> Result<()> {
    let collection_method = match args.method.as_str() {
        "ubus" => CollectionMethod::Ubus,
        "at" => CollectionMethod::At,
        "wifi" => CollectionMethod::Wifi,
        _ => CollectionMethod::Auto,
    };

    // Connect via ShellArgs (HTTP default, --ssh/--adb for shell)
    let (device, mut http_client) = {
        let connected = match args.shell.connect() {
            Ok(dev) => Some(dev),
            Err(_) if collection_method == CollectionMethod::Auto => None,
            Err(e) => return Err(e),
        };
        match connected {
            Some(DeviceShell::Http(client)) => (None, Some(client)),
            other => (other, None),
        }
    };

    if device.is_none() && http_client.is_none() {
        anyhow::bail!(
            "No data source available. Use -p <password> for HTTP, or --ssh/--adb for shell."
        );
    }

    // Create collector
    let mut collector = SignalCollector::new(collection_method);
    collector.probe(device.as_ref(), http_client.as_ref());

    let mut csv_logger = match &args.csv {
        Some(path) => Some(CsvLogger::new(path)?),
        None => None,
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let interval = Duration::from_secs(args.interval);
    let mut last_poll = Instant::now() - interval; // poll immediately
    let mut snapshot = SignalSnapshot::default();
    let mut devices_scroll: u16 = 0;
    let mut devices_panel_rect: Option<Rect> = None;
    let mut poll_count: u64 = 0;
    let start_time = Instant::now();
    let sources = collector.sources();

    loop {
        // Duration check
        if args.duration > 0 && start_time.elapsed() >= Duration::from_secs(args.duration) {
            break;
        }

        // Poll data if interval elapsed
        if last_poll.elapsed() >= interval {
            snapshot = collector.poll(device.as_ref(), http_client.as_mut());
            snapshot.device.monitoring_uptime_secs = Some(start_time.elapsed().as_secs());
            poll_count += 1;
            last_poll = Instant::now();
            devices_scroll = devices_scroll.min(snapshot.connected_devices.len().saturating_sub(1) as u16);

            if let Some(ref mut logger) = csv_logger {
                let _ = logger.log(&snapshot);
            }
        }

        // Draw UI
        terminal.draw(|f| {
            let size = f.area();

            // Determine active RAT for panel visibility
            let act = snapshot
                .cops
                .act
                .as_deref()
                .unwrap_or("")
                .to_uppercase();
            let has_nr_data = snapshot.nr.rsrp.is_some() || snapshot.nr.pci.is_some();
            let has_lte_data = snapshot.lte.rsrp.is_some() || snapshot.lte.pci.is_some();

            let act_hints_lte = act.contains("NSA") || act.contains("LTE") || act.contains("E-UTRAN")
                || act.contains("ENDC") || act.contains("EN-DC") || act == "4G" || act == "4G+";

            let show_nr = has_nr_data;
            let show_lte = has_lte_data && (act_hints_lte || act.is_empty() || show_nr);
            let show_3g = !show_nr
                && !show_lte
                && (snapshot.wcdma.rscp.is_some() || snapshot.wcdma.ecio.is_some());

            // Build dynamic vertical constraints
            let mut constraints: Vec<Constraint> = vec![
                Constraint::Length(3), // Header
            ];

            // Connected devices (if any)
            let has_devices = !snapshot.connected_devices.is_empty();
            if has_devices {
                let device_rows = snapshot.connected_devices.len() as u16 + 3; // border + header + rows
                let max_device_height = (size.height * 2 / 5).max(5);
                constraints.push(Constraint::Length(device_rows.min(max_device_height)));
            }

            // Unified cell panel (conditional)
            let show_cell = show_nr || show_lte || show_3g;
            if show_cell {
                let cell_height = if show_3g && !show_nr && !show_lte {
                    4  // 3G only: RSCP + Ec/Io + borders
                } else if show_nr && show_lte {
                    15 // NSA: Operator + Cell IDs + CAs + blank + 9 rows + borders
                } else if show_nr {
                    15 // SA: Operator + Cell ID + CA + blank + 9 rows + borders
                } else {
                    15 // LTE only: Operator + Cell ID + CA + blank + 9 rows + borders
                };
                constraints.push(Constraint::Length(cell_height));
            }

            // Bottom section: left (Device + SIM + WiFi) | right (Connection)
            constraints.push(Constraint::Min(10)); // bottom panels
            constraints.push(Constraint::Length(1)); // footer

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints.clone())
                .split(size);

            let mut chunk_idx = 0;

            // Header
            let elapsed = start_time.elapsed();
            let header_text = format!(
                " ZTE Signal Monitor | Sources: {} | Polls: {} | Uptime: {}:{:02}:{:02} | Interval: {}s{}",
                sources,
                poll_count,
                elapsed.as_secs() / 3600,
                (elapsed.as_secs() % 3600) / 60,
                elapsed.as_secs() % 60,
                args.interval,
                if args.csv.is_some() { " | CSV" } else { "" },
            );
            let header = Paragraph::new(header_text)
                .style(Style::default().fg(Color::White))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan))
                        .title(" zte monitor "),
                );
            f.render_widget(header, chunks[chunk_idx]);
            chunk_idx += 1;

            // Connected devices
            if has_devices {
                devices_panel_rect = Some(chunks[chunk_idx]);
                panels::render_devices_panel(f, chunks[chunk_idx], &snapshot.connected_devices, devices_scroll);
                chunk_idx += 1;
            } else {
                devices_panel_rect = None;
            }

            // Unified cell panel
            if show_cell {
                panels::render_cell_panel(
                    f, chunks[chunk_idx],
                    &snapshot.nr, &snapshot.lte, &snapshot.wcdma, &snapshot.cops,
                    show_nr, show_lte, show_3g,
                );
                chunk_idx += 1;
            }

            // Bottom section: side-by-side
            let bottom_area = chunks[chunk_idx];
            chunk_idx += 1;

            let bottom_cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(bottom_area);

            // Left column: Device + SIM + WiFi stacked
            let left_panels = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(8),  // Device
                    Constraint::Length(10), // SIM
                    Constraint::Min(6),    // WiFi (fills rest)
                ])
                .split(bottom_cols[0]);

            panels::render_device_panel(f, left_panels[0], &snapshot.device);
            panels::render_sim_panel(f, left_panels[1], &snapshot.device, &snapshot.cops, &snapshot.connection);
            panels::render_wifi_panel(f, left_panels[2], &snapshot.wifi);

            // Right column: Connection
            panels::render_connection_panel(f, bottom_cols[1], &snapshot.connection, args.debug);

            // Footer
            let footer = Paragraph::new(Line::from(vec![
                Span::styled(" q", Style::default().fg(Color::Yellow)),
                Span::raw("=quit  "),
                Span::styled("r", Style::default().fg(Color::Yellow)),
                Span::raw("=refresh  "),
                Span::styled("↑↓", Style::default().fg(Color::Yellow)),
                Span::raw("=scroll"),
            ]));
            f.render_widget(footer, chunks[chunk_idx]);
        })?;

        // Handle input events (non-blocking, 100ms timeout)
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('r') => {
                            last_poll = Instant::now() - interval;
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            let max = snapshot.connected_devices.len().saturating_sub(1) as u16;
                            devices_scroll = (devices_scroll + 1).min(max);
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            devices_scroll = devices_scroll.saturating_sub(1);
                        }
                        _ => {}
                    }
                }
                Event::Mouse(mouse) => {
                    let in_panel = devices_panel_rect
                        .map_or(false, |r| r.contains(Position::new(mouse.column, mouse.row)));
                    if in_panel {
                        match mouse.kind {
                            MouseEventKind::ScrollDown => {
                                let max = snapshot.connected_devices.len().saturating_sub(1) as u16;
                                devices_scroll = (devices_scroll + 1).min(max);
                            }
                            MouseEventKind::ScrollUp => {
                                devices_scroll = devices_scroll.saturating_sub(1);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    io::stdout().execute(DisableMouseCapture)?;
    io::stdout().execute(LeaveAlternateScreen)?;
    println!("Monitor stopped. {} polls completed.", poll_count);
    Ok(())
}
