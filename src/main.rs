use rusqlite::{params, Connection};
use teloxide::prelude::*;
use teloxide::types::BotCommand;

#[tokio::main]
async fn main() {
    env_logger::init();
    log::info!("Starte Tobus-Bot...");

    // 1. Datenbank initialisieren
    let conn = Connection::open("tobus_bot.db").expect("Konnte SQLite-Datenbank nicht öffnen");

    conn.execute(
        "CREATE TABLE IF NOT EXISTS todos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text TEXT NOT NULL,
            done INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )
    .expect("Konnte Tabelle nicht erstellen");

    log::info!("Datenbank für Tobus erfolgreich initialisiert.");

    // 2. Bot starten
    let bot = Bot::from_env();

    let commands = vec![
        BotCommand::new("start", "Startet Tobus und zeigt die Willkommensnachricht"),
        BotCommand::new("list", "Zeigt alle offenen Todos an"),
    ];

    // Sende die Liste an Telegram
    bot.set_my_commands(commands)
        .await
        .expect("Konnte Bot-Befehle nicht setzen");

    teloxide::repl(bot, move |bot: Bot, msg: Message| {
        let db_conn = Connection::open("tobus_bot.db").unwrap();

        async move {
            let user = match msg.from() {
                Some(u) => u,
                None => return respond(()),
            };

            // --- SICHERHEITS-CHECK: WHITELIST ---

            let my_telegram_id: u64 = std::env::var("MY_TELEGRAM_ID")
                .expect("MY_TELEGRAM_ID muss in der .env gesetzt sein")
                .parse()
                .expect("MY_TELEGRAM_ID muss eine gültige Zahl sein");

            if user.id.0 != my_telegram_id {
                log::warn!("Unbefugter Zugriff von User-ID: {}", user.id.0);
                bot.send_message(msg.chat.id, "Zugriff verweigert.").await?;
                return respond(());
            }

            if let Some(text) = msg.text() {
                if text.starts_with("/start") {
                    bot.send_message(
                        msg.chat.id,
                        "🤖 Hi! Ich bin **Tobus**, dein persönlicher Produktivitäts-Bot.\n\nSchicke mir einfach Text, um ein Todo zu speichern, oder /list, um deine Todos zu sehen.",
                    )
                    .await?;
                } else if text.starts_with("/list") {
                    // Wir lesen die Todos aus der Datenbank aus und fangen Fehler manuell ab
                    let todos_result = (|| {
                        let mut stmt = db_conn.prepare("SELECT id, text FROM todos WHERE done = 0")?;
                        let todo_iter = stmt.query_map([], |row| {
                            Ok((row.get::<_, i32>(0)?, row.get::<_, String>(1)?))
                        })?;

                        let mut vec = Vec::new();
                        for todo in todo_iter {
                            vec.push(todo?);
                        }
                        Ok::<Vec<(i32, String)>, rusqlite::Error>(vec)
                    })();

                    match todos_result {
                        Ok(todos) => {
                            let mut response = "📋 Deine offenen Todos:\n".to_string();
                            if todos.is_empty() {
                                response = "🎉 Keine offenen Todos! Du bist aktuell komplett frei.".to_string();
                            } else {
                                for (id, t) in todos {
                                    response.push_str(&format!("{}. {}\n", id, t));
                                }
                            }
                            bot.send_message(msg.chat.id, response).await?;
                        }
                        Err(e) => {
                            log::error!("Datenbankfehler beim Lesen: {}", e);
                            bot.send_message(msg.chat.id, "Fehler beim Laden der Todos.").await?;
                        }
                    }
                } else {
                    db_conn.execute(
                        "INSERT INTO todos (text) VALUES (?1)",
                        params![text],
                    ).unwrap();

                    bot.send_message(msg.chat.id, format!("✓ Tobus hat's notiert: {}", text))
                        .await?;

                    log::info!("Neues Todo gespeichert: {}", text);
                }
            }

            respond(())
        }
    })
    .await;
}
