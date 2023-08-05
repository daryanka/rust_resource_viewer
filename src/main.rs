use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{io, sync::Arc, time::Duration};
use sysinfo::NetworkExt;
use sysinfo::{CpuExt, ProcessExt, System, SystemExt};
use tokio::sync::RwLock;
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::Span,
    widgets::{
        Axis, BarChart, Block, BorderType, Borders, Cell, Chart, Dataset, GraphType, Paragraph,
        Row, Table,
    },
    Frame, Terminal,
};

#[derive(Debug)]
struct SystemData<'a> {
    system: System,
    total_memory: f64,
    memory_usage: Vec<f64>,
    memory_usage_as_tuple: Vec<(f64, f64)>,
    cpus: Vec<CPUData>,
    cpu_usage: f64,
    packets: [(&'a str, u64); 2],
    processes: Vec<Vec<String>>,
}

#[derive(Debug)]
struct CPUData {
    name: String,
    raw_data: Vec<f64>,
    data: Vec<(f64, f64)>,
    color: Color,
}

impl SystemData<'_> {
    fn update_system_info(&mut self) {
        self.system.refresh_all();

        // Memory
        self.memory_usage
            .push((self.system.used_memory() as f64) / 1024.0 / 1024.0 / 1024.0);

        if self.memory_usage.len() > 500 {
            self.memory_usage.remove(0);
        }

        self.total_memory = self.system.total_memory() as f64;
        self.memory_usage_as_tuple = create_tuple_vec_for_graph(&self.memory_usage);

        // CPU
        let all_cpus = vec![self.system.global_cpu_info()];
        // For future improvement to add individual cpu usage
        for (_, cpu) in all_cpus.iter().enumerate() {
            let cpu_name: String = format!("CPU {}", cpu.name());

            let cpu_vec = self.cpus.iter().position(|x| x.name == cpu_name);
            let cpu_index: usize = match cpu_vec {
                Some(index) => index,
                None => {
                    self.cpus.push(CPUData {
                        name: cpu_name.clone(),
                        raw_data: Vec::new(),
                        data: Vec::new(),
                        color: Color::Green,
                    });
                    self.cpus.len() - 1
                }
            };

            let cpu_vec = self.cpus.get_mut(cpu_index).unwrap();
            cpu_vec.raw_data.push(cpu.cpu_usage() as f64);
            if cpu_vec.raw_data.len() > 500 {
                cpu_vec.raw_data.remove(0);
            }
            cpu_vec.data = create_tuple_vec_for_graph(&cpu_vec.raw_data);
        }
        self.cpu_usage = self.system.global_cpu_info().cpu_usage() as f64;

        // Network
        let all_networks = self.system.networks();

        let (recieved_packets, transmitted_packets) = all_networks
            .into_iter()
            .map(|(_, net)| {
                return (net.packets_received(), net.packets_transmitted());
            })
            .reduce(|(a, b), (c, d)| {
                return (a + c, b + d);
            })
            .unwrap_or((0, 0));

        self.packets = [
            ("Packets In", recieved_packets),
            ("Packets Out", transmitted_packets),
        ];

        // Processes
        let num_cpus = self.system.cpus().len() as f32;
        let all_processes = self.system.processes();
        let mut sorted_processes = all_processes
            .iter()
            .map(|(_, p)| {
                return (
                    p.pid().to_string(),
                    p.name().to_owned(),
                    p.cpu_usage() / num_cpus,
                );
            })
            .collect::<Vec<(String, String, f32)>>();

        sorted_processes.sort_by(|a, b| {
            return a.2.partial_cmp(&b.2).unwrap();
        });

        // print first
        let top_processes = sorted_processes
            .iter()
            .rev()
            .take(100)
            .map(|(pid, name, cpu)| {
                return vec![pid.to_owned(), name.to_owned(), format!("{:.2}%", cpu)];
            })
            .collect::<Vec<Vec<String>>>();
        self.processes = top_processes;
    }
}

fn memory_to_gb(memory: &f64) -> String {
    format!("{:.2} GB", memory / 1024.0 / 1024. / 1024.0)
}

#[tokio::main]
async fn main() -> Result<()> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // run app
    let _ = run_app(&mut terminal).await;

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>) -> Result<()> {
    let poll_rate = 100;

    let mut state = SystemData {
        system: System::new_all(),
        total_memory: 0.0,
        memory_usage: Vec::new(),
        memory_usage_as_tuple: Vec::new(),
        cpus: Vec::new(),
        packets: [("Packets In", 0), ("Packets Out", 0)],
        processes: Vec::new(),
        cpu_usage: 0.0,
    };
    state.update_system_info();

    let system_data = Arc::new(RwLock::new(state));

    let loop_system_data = system_data.clone();
    tokio::spawn(async move {
        loop {
            loop_system_data.write().await.update_system_info();
            tokio::time::sleep(Duration::from_millis(poll_rate)).await;
        }
    });

    loop {
        let system_data = system_data.read().await;
        terminal.draw(|f| {
            ui(f, &system_data);
        })?;

        if event::poll(Duration::from_millis(poll_rate))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => {
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, system_data: &SystemData) {
    // Wrapping block for a group
    // Just draw the block and the group on the same area and build the group
    // with at least a margin of 1
    let size = f.size();

    // Surrounding block
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" System Info ")
        .title_alignment(Alignment::Left)
        .border_type(BorderType::Rounded);
    f.render_widget(block, size);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Percentage(48),
                Constraint::Percentage(48),
                Constraint::Max(3),
            ]
            .as_ref(),
        )
        .split(f.size());

    // Top two inner blocks
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .split(chunks[0]);

    let cpu_block = cpu_block(f, system_data, top_chunks[0]);
    f.render_widget(cpu_block, top_chunks[0]);

    let ram_block = ram_block(f, system_data, top_chunks[1]);
    f.render_widget(ram_block, top_chunks[1]);

    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .split(chunks[1]);

    let table = processes_block(system_data);
    f.render_widget(table, bottom_chunks[0]);

    let bar = network_block(system_data, bottom_chunks[1]);
    f.render_widget(bar, bottom_chunks[1]);

    let info_block = info_block();
    f.render_widget(info_block, chunks[2]);
}

fn ram_block<'a, B: Backend>(
    f: &mut Frame<B>,
    system_data: &'a SystemData,
    area: Rect,
) -> Chart<'a> {
    let block = Block::default()
        .title(" Memory Usage ")
        .borders(Borders::ALL);

    // create labels
    let x_labels = vec![Span::styled(
        "X AXIS",
        Style::default().add_modifier(Modifier::BOLD),
    )];

    let datasets = vec![Dataset::default()
        .marker(symbols::Marker::Dot)
        .style(Style::default().fg(Color::Cyan))
        .data(&system_data.memory_usage_as_tuple)];

    let c: Chart<'a> = Chart::new(datasets)
        .x_axis(
            Axis::default()
                .style(Style::default().fg(Color::Gray))
                .labels(x_labels)
                .bounds([1.0, 501.0]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(Color::Gray))
                .labels(vec![
                    Span::styled("0GB", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(
                        memory_to_gb(&system_data.total_memory),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ])
                .bounds([0.0, 100.0]),
        )
        .block(block);

    // add text inside area

    let percentage_used = format!(
        "{:.2}% Used",
        (system_data.system.used_memory() as f64) / system_data.total_memory * 100.0
    );
    let temp_rect = Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2);
    let widget = Paragraph::new(percentage_used).alignment(Alignment::Center);
    f.render_widget(widget, temp_rect);

    c
}

fn cpu_block<'a, B: Backend>(
    f: &mut Frame<B>,
    system_data: &'a SystemData,
    area: Rect,
) -> Chart<'a> {
    let block = Block::default().title(" CPU Usage ").borders(Borders::ALL);

    // create labels
    let x_labels = vec![Span::styled(
        "X AXIS",
        Style::default().add_modifier(Modifier::BOLD),
    )];

    let datasets = system_data
        .cpus
        .iter()
        .map(|item| {
            Dataset::default()
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(item.color))
                .data(&item.data)
        })
        .collect();

    let c: Chart<'a> = Chart::new(datasets)
        .x_axis(
            Axis::default()
                .style(Style::default().fg(Color::Gray))
                .labels(x_labels)
                .bounds([1.0, 501.0]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(Color::Gray))
                .labels(vec![
                    Span::styled("0%", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled("100%", Style::default().add_modifier(Modifier::BOLD)),
                ])
                .bounds([0.0, 100.0]),
        )
        .block(block);

    // add text inside area

    let percentage_used = format!("{:.2}%", system_data.cpu_usage,);
    let temp_rect = Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2);
    let widget = Paragraph::new(percentage_used).alignment(Alignment::Center);
    f.render_widget(widget, temp_rect);

    c
}

fn network_block<'a>(system_data: &'a SystemData, area: Rect) -> BarChart<'a> {
    let block = Block::default()
        .title(" Network Usage ")
        .borders(Borders::ALL);

    // max of 2 bars
    let calc_bar_width = area.width / 2 - 3;
    let max = {
        let mut max = 0;
        for (_, v) in system_data.packets.iter() {
            if *v > max {
                max = *v;
            }
        }
        max
    };

    let bar = BarChart::default()
        .block(block)
        .bar_width(calc_bar_width)
        .bar_gap(2)
        .bar_style(Style::default().fg(Color::Yellow))
        .label_style(Style::default().fg(Color::White))
        .data(&system_data.packets)
        .max(max);
    bar
}

fn processes_block<'a>(system_data: &'a SystemData) -> Table<'a> {
    let block = Block::default().title(" Processes ").borders(Borders::ALL);

    let selected_style = Style::default().add_modifier(Modifier::REVERSED);

    let header_cells = ["PID", "Process Name", "Usage"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default()));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = system_data.processes.iter().map(|item| {
        let height = item
            .iter()
            .map(|content| content.chars().filter(|c| *c == '\n').count())
            .max()
            .unwrap_or(0)
            + 1;
        let cells = item.iter().map(|c| Cell::from(c.clone()));
        Row::new(cells).height(height as u16).bottom_margin(1)
    });

    let t = Table::new(rows)
        .header(header)
        .block(block)
        .highlight_style(selected_style)
        .highlight_symbol(">> ")
        .widths(&[
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ]);
    t
}

fn info_block() -> Paragraph<'static> {
    let block = Block::default().title(" Usage ").borders(Borders::ALL);
    Paragraph::new("quit: q")
        .alignment(Alignment::Left)
        .block(block)
}

// This is really hacky and probably not the best way to do this
fn create_tuple_vec_for_graph(data: &Vec<f64>) -> Vec<(f64, f64)> {
    let mut result = Vec::new();
    for (i, d) in data.iter().enumerate() {
        result.push(((i + 1) as f64, *d));
    }
    result
}
